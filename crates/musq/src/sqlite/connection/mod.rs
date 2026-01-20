use std::{
    fmt::{self, Debug, Formatter, Write},
    result::Result as StdResult,
    sync::atomic::Ordering,
};

use either::Either;
use futures_core::{future::BoxFuture, stream::BoxStream};
use futures_util::{FutureExt, StreamExt, TryFutureExt, TryStreamExt, future};
pub use handle::ConnectionHandle;

use crate::{
    QueryResult, Result, Row,
    error::Error,
    executor::Execute,
    logger::LogSettings,
    musq::{Musq, OptimizeOnClose},
    sqlite::{
        connection::{establish::EstablishParams, worker::ConnectionWorker},
        statement::{Prepared, Statement},
    },
    statement_cache::StatementCache,
    transaction::Transaction,
};
/// Connection establishment helpers.
pub mod establish;
/// Query execution helpers for connections.
pub mod execute;

// removed executor trait implementation module
/// Low-level connection handle.
mod handle;
/// Worker task driving the connection.
mod worker;

/// A single, standalone connection to a SQLite database.
///
/// This represents a single physical connection and is the fundamental primitive for database
/// interaction. It is created by calling [`Connection::connect_with()`].
///
/// For applications with concurrent database access, it is recommended to use a [`Pool`]
/// instead of managing `Connection` objects directly. The `Pool` provides managed, reusable
/// connections via [`PoolConnection`].
///
/// However, for simple applications, scripts, or any scenario where connection pooling is
/// unnecessary, a standalone `Connection` is the most direct way to interact with the database.
///
/// ### Transactions
///
/// A `Connection` can be used to start a new transaction by calling
/// [`connection.begin()`][Connection::begin].
///
/// ### Closing
///
/// When a `Connection` is dropped, it is closed. To handle potential errors on close, it is
/// recommended to explicitly call the [`close()`] method.
pub struct Connection {
    /// Optimize-on-close behavior.
    optimize_on_close: OptimizeOnClose,
    /// Background worker thread.
    pub(crate) worker: ConnectionWorker,
    /// Size of the row channel.
    pub(crate) row_channel_size: usize,
}

// Connection is safe to share between threads because:
// - optimize_on_close is just an enum, safe to share
// - worker is ConnectionWorker which we've marked as Sync
// - row_channel_size is just a usize, safe to share
unsafe impl Sync for Connection {}

/// Internal state for an active connection.
pub struct ConnectionState {
    /// Low-level SQLite handle.
    pub(crate) handle: ConnectionHandle,

    // transaction status
    /// Current nested transaction depth.
    pub(crate) transaction_depth: usize,

    /// Cached prepared statements.
    pub(crate) statements: StatementCache,

    /// Logging configuration.
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
    /// Establish a new connection from provided options.
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
    pub async fn close(&self) -> Result<()> {
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

    /// Return the current cached statement count.
    pub(crate) fn cached_statements_size(&self) -> usize {
        self.worker
            .shared
            .cached_statements_size
            .load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) async fn clear_cached_statements(&self) -> Result<()> {
        self.worker.clear_cache().await?;
        Ok(())
    }

    /// Execute the function inside a transaction.
    ///
    /// If the function returns an error, the transaction will be rolled back. If it does not
    /// return an error, the transaction will be committed.
    pub async fn transaction<'a, F, R, E>(&'a mut self, callback: F) -> StdResult<R, E>
    where
        for<'c> F: FnOnce(&'c mut Transaction<&'a mut Self>) -> BoxFuture<'c, StdResult<R, E>>
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
    /// Execute a query and stream both rows and results.
    pub(crate) fn fetch_many<'c, 'q: 'c, E>(
        &'c self,
        query: E,
    ) -> BoxStream<'c, Result<Either<QueryResult, Row>>>
    where
        E: Execute + 'q,
    {
        let mut query = query;
        let arguments = query.arguments();
        let sql = query.sql().into();
        drop(query);

        Box::pin(
            self.worker
                .execute(sql, arguments, self.row_channel_size)
                .map_ok(flume::Receiver::into_stream)
                .try_flatten_stream(),
        )
    }

    /// Execute a query and return the first row if present.
    pub(crate) fn fetch_optional<'c, 'q: 'c, E>(
        &'c self,
        query: E,
    ) -> BoxFuture<'c, Result<Option<Row>>>
    where
        E: Execute + 'q,
    {
        let mut query = query;
        let arguments = query.arguments();
        let sql = query.sql().to_string();
        drop(query);

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

    #[allow(dead_code)]
    /// Prepare a SQL statement without caching.
    pub(crate) fn prepare_with<'c, 'q: 'c>(
        &'c self,
        sql: &'q str,
    ) -> BoxFuture<'c, Result<Prepared>> {
        Box::pin(async move {
            self.worker.prepare(sql).await?;

            Ok(Prepared {
                statement: Statement { sql: sql.into() },
            })
        })
    }

    /// Prepare a SQL statement using the cache.
    pub fn prepare<'c, 'q: 'c>(&'c self, sql: &'q str) -> BoxFuture<'c, Result<Prepared>> {
        self.prepare_with(sql)
    }

    /// Execute a query and stream only rows.
    pub(crate) fn fetch<'c, 'q: 'c, E>(&'c self, query: E) -> BoxStream<'c, Result<Row>>
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

    /// Execute a query and stream only result summaries.
    pub(crate) fn execute_many<'c, 'q: 'c, E>(
        &'c self,
        query: E,
    ) -> BoxStream<'c, Result<QueryResult>>
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

    /// Execute a query and return a combined result summary.
    pub(crate) fn execute<'c, 'q: 'c, E>(&'c self, query: E) -> BoxFuture<'c, Result<QueryResult>>
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

    /// Execute a query and collect all rows.
    pub(crate) fn fetch_all<'c, 'q: 'c, E>(&'c self, query: E) -> BoxFuture<'c, Result<Vec<Row>>>
    where
        E: Execute + 'q,
    {
        self.fetch(query).try_collect().boxed()
    }

    /// Execute a query and return exactly one row.
    pub(crate) fn fetch_one<'c, 'q: 'c, E>(&'c self, query: E) -> BoxFuture<'c, Result<Row>>
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
        // Drop is called when Connection is being destroyed,
        // so we don't need to properly shut down the worker here
        // The worker thread will naturally terminate when the command channel is dropped
    }
}

impl Drop for ConnectionState {
    fn drop(&mut self) {
        // explicitly drop statements before the connection handle is dropped
        self.statements.clear();
    }
}
