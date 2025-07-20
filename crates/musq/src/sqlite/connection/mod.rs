use std::fmt::{self, Debug, Formatter, Write};

use futures_core::{future::BoxFuture, stream::BoxStream};
use futures_executor::block_on;
use futures_util::{FutureExt, StreamExt, TryFutureExt, TryStreamExt, future};

use either::Either;

use crate::{
    QueryResult, Result, Row, Statement,
    error::Error,
    executor::Execute,
    logger::LogSettings,
    musq::{Musq, OptimizeOnClose},
    sqlite::connection::{establish::EstablishParams, worker::ConnectionWorker},
    statement_cache::StatementCache,
    transaction::Transaction,
};

pub(crate) use handle::ConnectionHandle;
pub(crate) mod establish;
pub(crate) mod execute;

// removed executor trait implementation module
mod handle;
mod worker;

/// A connection to an open [Sqlite] database.
///
/// Because SQLite is an in-process database accessed by blocking API calls, Musq uses a background
/// thread and communicates with it via channels to allow non-blocking access to the database.
///
/// Dropping this struct closes the connection immediately by signalling the worker thread to
/// quit and close the database. If an error occurs there is no way to pass it back to the
/// user this way.
///
/// You can explicitly call [`.close()`][Self::close] to ensure the database is closed successfully
/// or get an error otherwise.
pub struct Connection {
    optimize_on_close: OptimizeOnClose,
    pub(crate) worker: ConnectionWorker,
    pub(crate) row_channel_size: usize,
}

pub(crate) struct ConnectionState {
    pub(crate) handle: ConnectionHandle,

    // transaction status
    pub(crate) transaction_depth: usize,

    pub(crate) statements: StatementCache,

    log_settings: LogSettings,
}

impl Debug for Connection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteConnection")
            .field("row_channel_size", &self.row_channel_size)
            .field("cached_statements_size", &self.cached_statements_size())
            .finish()
    }
}

impl Connection {
    pub(crate) async fn establish(options: &Musq) -> Result<Self> {
        let params = EstablishParams::from_options(options)?;
        let worker = ConnectionWorker::establish(params).await?;
        Ok(Self {
            optimize_on_close: options.optimize_on_close.clone(),
            worker,
            row_channel_size: options.row_channel_size,
        })
    }

    /// Explicitly close this database connection.
    ///
    /// This notifies the database server that the connection is closing so that it can
    /// free up any server-side resources in use.
    ///
    /// While connections can simply be dropped to clean up local resources,
    /// the `Drop` handler itself cannot notify the server that the connection is being closed
    /// because that may require I/O to send a termination message. That can result in a delay
    /// before the server learns that the connection is gone, usually from a TCP keepalive timeout.
    ///
    /// Creating and dropping many connections in short order without calling `.close()` may
    /// lead to errors from the database server because those senescent connections will still
    /// count against any connection limit or quota that is configured.
    ///
    /// Therefore it is recommended to call `.close()` on a connection when you are done using it
    /// and to `.await` the result to ensure the termination message is sent.
    ///
    /// The returned future **must** be awaited to ensure the connection is fully
    /// closed.
    #[must_use = "futures returned by `Connection::close` must be awaited"]
    pub async fn close(mut self) -> Result<()> {
        if let OptimizeOnClose::Enabled { analysis_limit } = self.optimize_on_close {
            let mut pragma_string = String::new();
            if let Some(limit) = analysis_limit {
                write!(pragma_string, "PRAGMA analysis_limit = {limit}; ").ok();
            }
            pragma_string.push_str("PRAGMA optimize;");
            self.execute(crate::query(&pragma_string)).await?;
        }
        self.worker.shutdown().await
    }

    /// Begin a new transaction or establish a savepoint within the active transaction.
    ///
    /// Returns a [`Transaction`] for controlling and tracking the new transaction.
    pub fn begin(&mut self) -> BoxFuture<'_, Result<Transaction<&mut Self>>>
    where
        Self: Sized,
    {
        Transaction::begin(self)
    }

    pub(crate) fn cached_statements_size(&self) -> usize {
        self.worker
            .shared
            .cached_statements_size
            .load(std::sync::atomic::Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) async fn clear_cached_statements(&mut self) -> Result<()> {
        self.worker.clear_cache().await?;
        Ok(())
    }

    /// Execute the function inside a transaction.
    ///
    /// If the function returns an error, the transaction will be rolled back. If it does not
    /// return an error, the transaction will be committed.
    pub async fn transaction<'a, F, R, E>(&'a mut self, callback: F) -> std::result::Result<R, E>
    where
        for<'c> F: FnOnce(&'c mut Transaction<&'a mut Self>) -> BoxFuture<'c, std::result::Result<R, E>>
            + Send
            + Sync,
        Self: Sized,
        R: Send,
        E: From<Error> + Send,
    {
        let mut transaction = self.begin().await?;
        let ret = {
            let fut = callback(&mut transaction);
            fut.await
        };

        match ret {
            Ok(ret) => {
                transaction.commit().await?;

                Ok(ret)
            }
            Err(err) => {
                transaction.rollback().await?;
                Err(err)
            }
        }
    }

    /// Establish a new database connection with the provided options.
    pub async fn connect_with(options: &Musq) -> Result<Self>
    where
        Self: Sized,
    {
        options.connect().await
    }
}

impl Connection {
    pub fn fetch_many<'c, 'q: 'c, E>(
        &'c mut self,
        mut query: E,
    ) -> BoxStream<'c, Result<Either<QueryResult, Row>>>
    where
        E: Execute + 'q,
    {
        let arguments = query.take_arguments();
        let sql = query.sql().into();

        Box::pin(
            self.worker
                .execute(sql, arguments, self.row_channel_size)
                .map_ok(flume::Receiver::into_stream)
                .try_flatten_stream(),
        )
    }

    pub fn fetch_optional<'c, 'q: 'c, E>(
        &'c mut self,
        mut query: E,
    ) -> BoxFuture<'c, Result<Option<Row>>>
    where
        E: Execute + 'q,
    {
        let arguments = query.take_arguments();
        let sql = query.sql().to_string();

        Box::pin(async move {
            let stream = self
                .worker
                .execute(sql, arguments, self.row_channel_size)
                .map_ok(flume::Receiver::into_stream)
                .try_flatten_stream();

            futures_util::pin_mut!(stream);

            while let Some(res) = stream.try_next().await? {
                if let Either::Right(row) = res {
                    return Ok(Some(row));
                }
            }

            Ok(None)
        })
    }

    pub fn prepare_with<'c, 'q: 'c>(
        &'c mut self,
        sql: &'q str,
    ) -> BoxFuture<'c, Result<Statement>> {
        Box::pin(async move {
            let statement = self.worker.prepare(sql).await?;

            Ok(Statement {
                sql: sql.into(),
                ..statement
            })
        })
    }

    pub fn prepare<'c, 'q: 'c>(&'c mut self, sql: &'q str) -> BoxFuture<'c, Result<Statement>> {
        self.prepare_with(sql)
    }

    pub fn fetch<'c, 'q: 'c, E>(&'c mut self, query: E) -> BoxStream<'c, Result<Row>>
    where
        E: Execute + 'q,
    {
        self.fetch_many(query)
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(_) => None,
                    Either::Right(row) => Some(row),
                })
            })
            .boxed()
    }

    pub fn execute_many<'c, 'q: 'c, E>(&'c mut self, query: E) -> BoxStream<'c, Result<QueryResult>>
    where
        E: Execute + 'q,
    {
        self.fetch_many(query)
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(rows) => Some(rows),
                    Either::Right(_) => None,
                })
            })
            .boxed()
    }

    pub fn execute<'c, 'q: 'c, E>(&'c mut self, query: E) -> BoxFuture<'c, Result<QueryResult>>
    where
        E: Execute + 'q,
    {
        self.execute_many(query)
            .try_fold(QueryResult::default(), |mut acc, qr| async move {
                acc.changes += qr.changes;
                acc.last_insert_rowid = qr.last_insert_rowid;
                Ok(acc)
            })
            .boxed()
    }

    pub fn fetch_all<'c, 'q: 'c, E>(&'c mut self, query: E) -> BoxFuture<'c, Result<Vec<Row>>>
    where
        E: Execute + 'q,
    {
        self.fetch(query).try_collect().boxed()
    }

    pub fn fetch_one<'c, 'q: 'c, E>(&'c mut self, query: E) -> BoxFuture<'c, Result<Row>>
    where
        E: Execute + 'q,
    {
        self.fetch_optional(query)
            .and_then(|row| match row {
                Some(row) => future::ok(row),
                None => future::err(Error::RowNotFound),
            })
            .boxed()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // best effort shutdown of the worker thread
        if !self.worker.is_shutdown() {
            let _ = block_on(self.worker.shutdown());
        }
    }
}

impl Drop for ConnectionState {
    fn drop(&mut self) {
        // explicitly drop statements before the connection handle is dropped
        self.statements.clear();
    }
}
