use std::{
    fmt::{self, Debug, Formatter, Write},
    os::raw::c_void,
    panic::catch_unwind,
    ptr::NonNull,
};

use crate::sqlite::ffi;
use futures_core::{future::BoxFuture, stream::BoxStream};
use futures_util::{FutureExt, StreamExt, TryFutureExt, TryStreamExt, future};
use libsqlite3_sys::sqlite3;
use tokio::sync::MutexGuard;

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
/// Dropping this struct will signal the worker thread to quit and close the database, though
/// if an error occurs there is no way to pass it back to the user this way.
///
/// You can explicitly call [`.close()`][Self::close] to ensure the database is closed successfully
/// or get an error otherwise.
pub struct Connection {
    optimize_on_close: OptimizeOnClose,
    pub(crate) worker: ConnectionWorker,
    pub(crate) row_channel_size: usize,
}

pub struct LockedSqliteHandle<'a> {
    pub(crate) guard: MutexGuard<'a, ConnectionState>,
}

/// Represents a callback handler that will be shared with the underlying sqlite3 connection.
pub(crate) struct Handler(NonNull<dyn FnMut() -> bool + Send + 'static>);
unsafe impl Send for Handler {}

pub(crate) struct ConnectionState {
    pub(crate) handle: ConnectionHandle,

    // transaction status
    pub(crate) transaction_depth: usize,

    pub(crate) statements: StatementCache,

    log_settings: LogSettings,

    /// Stores the progress handler set on the current connection. If the handler returns `false`,
    /// the query is interrupted.
    progress_handler_callback: Option<Handler>,
}

impl ConnectionState {
    /// Drops the `progress_handler_callback` if it exists.
    pub(crate) fn remove_progress_handler(&mut self) {
        if let Some(handler) = self.progress_handler_callback.take() {
            unsafe {
                ffi::progress_handler(self.handle.as_ptr(), 0, None, std::ptr::null_mut());
                let _ = { Box::from_raw(handler.0.as_ptr()) };
            }
        }
    }
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

    /// Lock the SQLite database handle out from the worker thread so direct SQLite API calls can
    /// be made safely.
    ///
    /// Returns an error if the worker thread crashed.
    pub async fn lock_handle(&mut self) -> Result<LockedSqliteHandle<'_>> {
        let guard = self.worker.unlock_db().await?;

        Ok(LockedSqliteHandle { guard })
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
    pub async fn close(mut self) -> Result<()> {
        if let OptimizeOnClose::Enabled { analysis_limit } = self.optimize_on_close {
            let mut pragma_string = String::new();
            if let Some(limit) = analysis_limit {
                write!(pragma_string, "PRAGMA analysis_limit = {limit}; ").ok();
            }
            pragma_string.push_str("PRAGMA optimize;");
            self.execute(crate::query(&pragma_string)).await?;
        }
        // Destructure self to extract the worker and avoid partial move
        let Connection { mut worker, .. } = self;
        let shutdown = worker.shutdown();
        // The rest of self is dropped here
        // Ensure the worker thread has terminated
        shutdown.await
    }

    /// Immediately close the connection without sending a graceful shutdown.
    pub async fn close_hard(self) -> Result<()> {
        drop(self);
        Ok(())
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

    pub fn cached_statements_size(&self) -> usize {
        self.worker
            .shared
            .cached_statements_size
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub async fn clear_cached_statements(&mut self) -> Result<()> {
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
        self.execute_many(query).try_collect().boxed()
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

/// Implements a C binding to a progress callback. The function returns `0` if the
/// user-provided callback returns `true`, and `1` otherwise to signal an interrupt.
extern "C" fn progress_callback<F>(callback: *mut c_void) -> i32
where
    F: FnMut() -> bool,
{
    unsafe {
        let r = catch_unwind(|| {
            let callback: *mut F = callback.cast::<F>();
            (*callback)()
        });
        i32::from(!r.unwrap_or_default())
    }
}

impl LockedSqliteHandle<'_> {
    /// Returns the underlying sqlite3* connection handle.
    ///
    /// As long as this `LockedSqliteHandle` exists, it is guaranteed that the background thread
    /// is not making FFI calls on this database handle or any of its statements.
    ///
    /// ### Note: The `sqlite3` type is semver-exempt.
    /// This API exposes the `sqlite3` type from `libsqlite3-sys` crate for type safety.
    /// However, we reserve the right to upgrade `libsqlite3-sys` as necessary.
    ///
    /// Thus, if you are making direct calls via `libsqlite3-sys` you should pin the version
    /// of Musq that you're using, and upgrade it and `libsqlite3-sys` manually as new
    /// versions are released.
    pub fn as_raw_handle(&mut self) -> NonNull<sqlite3> {
        self.guard.handle.as_non_null_ptr()
    }

    /// Sets a progress handler that is invoked periodically during long running calls. If the progress callback
    /// returns `false`, then the operation is interrupted.
    ///
    /// `num_ops` is the approximate number of [virtual machine instructions](https://www.sqlite.org/opcode.html)
    /// that are evaluated between successive invocations of the callback. If `num_ops` is less than one then the
    /// progress handler is disabled.
    ///
    /// Only a single progress handler may be defined at one time per database connection; setting a new progress
    /// handler cancels the old one.
    ///
    /// The progress handler callback must not do anything that will modify the database connection that invoked
    /// the progress handler. Note that sqlite3_prepare_v2() and sqlite3_step() both modify their database connections
    /// in this context.
    pub fn set_progress_handler<F>(&mut self, num_ops: i32, callback: F)
    where
        F: FnMut() -> bool + Send + 'static,
    {
        unsafe {
            let callback_boxed = Box::new(callback);
            // SAFETY: `Box::into_raw()` always returns a non-null pointer.
            let callback = NonNull::new_unchecked(Box::into_raw(callback_boxed));
            let handler = callback.as_ptr() as *mut _;
            self.guard.remove_progress_handler();
            self.guard.progress_handler_callback = Some(Handler(callback));

            ffi::progress_handler(
                self.as_raw_handle().as_mut(),
                num_ops,
                Some(progress_callback::<F>),
                handler,
            );
        }
    }

    /// Removes the progress handler on a database connection. The method does nothing if no handler was set.
    pub fn remove_progress_handler(&mut self) {
        self.guard.remove_progress_handler();
    }
}

impl Drop for ConnectionState {
    fn drop(&mut self) {
        // explicitly drop statements before the connection handle is dropped
        self.statements.clear();
        self.remove_progress_handler();
    }
}
