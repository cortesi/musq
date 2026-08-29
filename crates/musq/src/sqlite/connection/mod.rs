use std::{
    ffi::CString,
    fmt::{self, Debug, Formatter},
    io,
    ops::AsyncFnOnce,
    path::Path,
    result::Result as StdResult,
    sync::atomic::Ordering,
};

pub use control::{
    DbStatus, DbStatusKind, ForeignKeyViolation, IntegrityReport, SqliteRuntimeInfo, WalCheckpoint,
    WalCheckpointMode,
};
use either::Either;
use futures_core::{future::BoxFuture, stream::BoxStream};
use futures_util::{FutureExt, StreamExt, TryFutureExt, TryStreamExt, future};
pub use handle::ConnectionHandle;
pub use worker::InterruptHandle;

use crate::{
    QueryResult, Result, Row,
    error::Error,
    executor::Execute,
    logger::LogSettings,
    musq::Musq,
    query::{query_as, query_scalar},
    sqlite::{
        connection::{establish::EstablishParams, worker::ConnectionWorker},
        ffi,
    },
    statement_cache::StatementCache,
    transaction::{Transaction, TxnState},
};
/// Connection diagnostics and control helpers.
mod control;
/// Connection establishment helpers.
pub mod establish;
/// Query execution helpers for connections.
pub mod execute;

// removed executor trait implementation module
/// Low-level connection handle.
mod handle;
/// Worker task driving the connection.
mod worker;

/// How [`Connection::deserialize`] treats the loaded image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeserializeMode {
    /// Load the image as a read-only database.
    ReadOnly,
    /// Load the image so SQLite may grow the buffer on write.
    Resizable,
}

/// Progress from [`Connection::backup_to_path`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupReport {
    /// Total pages in the backup source.
    pub pages: i32,
    /// Pages still to copy after the last step. Zero when the copy finished.
    pub remaining: i32,
}

/// A single, standalone connection to a SQLite database.
///
/// This represents a single physical connection and is the fundamental primitive for database
/// interaction. It is created by calling [`Connection::connect_with()`].
///
/// For applications with concurrent database access, it is recommended to use a [`crate::Pool`]
/// instead of managing `Connection` objects directly. The `Pool` provides managed, reusable
/// connections via [`crate::PoolConnection`].
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
/// recommended to explicitly call the [`Connection::close`] method.
pub struct Connection {
    /// Optimize-on-close behavior.
    optimize_on_close: bool,
    /// Background worker thread.
    pub(crate) worker: ConnectionWorker,
    /// Size of the row channel.
    pub(crate) row_channel_size: usize,
    /// Default start mode for [`Connection::begin`].
    pub(crate) default_transaction_behavior: crate::TransactionBehavior,
}

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
            optimize_on_close: options.optimize_on_close,
            worker,
            row_channel_size: options.row_channel_size,
            default_transaction_behavior: options.default_transaction_behavior,
        })
    }

    /// Close this connection and shut down its worker thread.
    ///
    /// This consumes the connection so later calls cannot observe a closed handle.
    /// Dropping a connection also shuts the worker down when the command channel
    /// is dropped. Call `close` when you need to wait for that shutdown and handle
    /// errors from `PRAGMA optimize` when optimize-on-close is enabled.
    ///
    /// The returned future **must** be awaited to ensure the connection is fully
    /// closed.
    #[must_use = "futures returned by `Connection::close` must be awaited"]
    pub async fn close(self) -> Result<()> {
        if self.optimize_on_close {
            self.execute(crate::query("PRAGMA optimize;")).await?;
        }
        self.worker.shutdown().await
    }

    /// Begin a new transaction or establish a savepoint within the active transaction.
    ///
    /// Uses the connection's default [`crate::TransactionBehavior`]. Nested
    /// calls create savepoints.
    pub fn begin(&mut self) -> BoxFuture<'_, Result<Transaction<&mut Self>>>
    where
        Self: Sized,
    {
        let behavior = self.default_transaction_behavior;
        Transaction::begin_with(self, behavior)
    }

    /// Begin a transaction with an explicit start mode.
    ///
    /// Nested calls still create savepoints.
    pub fn begin_with(
        &mut self,
        behavior: crate::TransactionBehavior,
    ) -> BoxFuture<'_, Result<Transaction<&mut Self>>>
    where
        Self: Sized,
    {
        Transaction::begin_with(self, behavior)
    }

    /// Return whether SQLite currently has no explicit transaction open.
    pub async fn is_autocommit(&self) -> Result<bool> {
        self.worker.is_autocommit().await
    }

    /// Return SQLite's transaction lock state for this connection.
    ///
    /// This reports `NONE`, `READ`, or `WRITE` from `sqlite3_txn_state`.
    /// It does not report savepoint nesting. Use [`Self::is_autocommit`]
    /// to see whether an explicit transaction is open.
    pub async fn transaction_state(&self) -> Result<TxnState> {
        self.worker.transaction_state().await
    }

    /// Serialize `schema` to a SQLite database image.
    ///
    /// The image is a copy of the named schema, usually `"main"`. SQLite
    /// allocates the buffer; Musq copies it into a `Vec` and frees the
    /// original.
    pub async fn serialize(&self, schema: &str) -> Result<Vec<u8>> {
        let schema = CString::new(schema)
            .map_err(|_| Error::Configuration("serialize schema contains nul bytes".into()))?;
        self.worker.serialize(schema).await
    }

    /// Replace `schema` with a SQLite database image.
    ///
    /// Refuses to run while a transaction is open. Cached statements are
    /// cleared because they bind to the previous schema. WAL-mode images
    /// are rejected. `mode` chooses a read-only load or a resizable buffer.
    /// SQLite takes ownership of a copy allocated with `sqlite3_malloc64`.
    pub async fn deserialize(
        &self,
        schema: &str,
        bytes: Vec<u8>,
        mode: DeserializeMode,
    ) -> Result<()> {
        let schema = CString::new(schema)
            .map_err(|_| Error::Configuration("deserialize schema contains nul bytes".into()))?;
        self.worker.deserialize(schema, bytes, mode).await
    }

    /// Copy this database to `path` with the SQLite backup API.
    ///
    /// The worker opens the destination on its own thread, copies
    /// `pages_per_step` pages per step, and calls `backup_finish` on every
    /// exit path. `pages_per_step` of zero copies all remaining pages in one
    /// step. The destination path must not be the source file.
    pub async fn backup_to_path(
        &self,
        path: impl AsRef<Path>,
        pages_per_step: u32,
    ) -> Result<BackupReport> {
        let path = path.as_ref();
        let dest = path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "filename passed to SQLite must be valid UTF-8",
            )
        })?;
        let dest = CString::new(dest).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "filename passed to SQLite must not contain nul bytes",
            )
        })?;
        let pages_per_step = i32::try_from(pages_per_step).unwrap_or(i32::MAX);
        self.worker
            .backup_to_path(dest, path.to_path_buf(), pages_per_step)
            .await
    }

    /// Interrupt the statement currently running on this connection.
    ///
    /// The in-flight statement fails with `SQLITE_INTERRUPT`. If that
    /// statement was a write inside an explicit transaction, SQLite rolls
    /// the transaction back. The next `commit` or `rollback` then returns
    /// [`Error::TransactionAborted`]. Later statements run normally.
    ///
    /// This method is safe to call from another thread while a query runs.
    /// Use [`Self::interrupt_handle`] when you need to interrupt after this
    /// connection has been moved into `close`.
    pub fn interrupt(&self) {
        self.worker.interrupt();
    }

    /// Return a cloneable handle that can interrupt this connection.
    ///
    /// A call after the worker has closed the database pointer is a no-op.
    pub fn interrupt_handle(&self) -> InterruptHandle {
        self.worker.interrupt_handle()
    }

    /// Return the current cached statement count.
    pub(crate) fn cached_statements_size(&self) -> usize {
        self.worker
            .shared
            .cached_statements_size
            .load(Ordering::Acquire)
    }

    /// Return runtime identity and compile options for this connection.
    pub async fn runtime_info(&self) -> Result<SqliteRuntimeInfo> {
        let version: String = query_scalar("SELECT sqlite_version()")
            .fetch_one(self)
            .await?;
        let source_id: String = query_scalar("SELECT sqlite_source_id()")
            .fetch_one(self)
            .await?;
        let compile_option_rows: Vec<(String,)> =
            query_as("PRAGMA compile_options").fetch_all(self).await?;
        let mut compile_options = compile_option_rows
            .into_iter()
            .map(|(option,)| option)
            .collect::<Vec<_>>();
        compile_options.sort();

        Ok(SqliteRuntimeInfo {
            version,
            version_number: ffi::libversion_number(),
            source_id,
            compile_options,
        })
    }

    /// Run SQLite's full integrity check on this connection.
    pub async fn integrity_check(&self) -> Result<IntegrityReport> {
        self.integrity_report("PRAGMA integrity_check").await
    }

    /// Run SQLite's faster integrity check on this connection.
    pub async fn quick_check(&self) -> Result<IntegrityReport> {
        self.integrity_report("PRAGMA quick_check").await
    }

    /// Return all foreign-key violations visible to this connection.
    pub async fn foreign_key_check(&self) -> Result<Vec<ForeignKeyViolation>> {
        let rows: Vec<(String, Option<i64>, String, i64)> =
            query_as("PRAGMA foreign_key_check").fetch_all(self).await?;
        Ok(rows
            .into_iter()
            .map(
                |(table, row_id, parent, foreign_key_index)| ForeignKeyViolation {
                    table,
                    row_id,
                    parent,
                    foreign_key_index,
                },
            )
            .collect())
    }

    /// Collect each message from one SQLite integrity pragma.
    async fn integrity_report(&self, pragma: &str) -> Result<IntegrityReport> {
        let messages = query_scalar(pragma).fetch_all(self).await?;
        Ok(IntegrityReport { messages })
    }

    /// Return a per-connection SQLite status counter.
    ///
    /// If `reset_highwater` is true, SQLite resets the high-water mark after
    /// reading it for counters that support reset.
    pub async fn db_status(&self, kind: DbStatusKind, reset_highwater: bool) -> Result<DbStatus> {
        self.worker.db_status(kind, reset_highwater).await
    }

    /// Run a WAL checkpoint or inspect WAL status for an attached database.
    ///
    /// Pass `None` for SQLite's default schema behavior, or `Some("main")` for
    /// the primary database.
    pub async fn wal_checkpoint(
        &self,
        schema: Option<&str>,
        mode: WalCheckpointMode,
    ) -> Result<WalCheckpoint> {
        let schema = schema
            .map(CString::new)
            .transpose()
            .map_err(|_| Error::Configuration("WAL checkpoint schema contains nul bytes".into()))?;

        self.worker.wal_checkpoint(schema, mode).await
    }

    /// Return this connection's configured parser stack depth limit.
    pub async fn parser_depth_limit(&self) -> Result<u32> {
        self.worker.parser_depth_limit().await
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
    pub async fn transaction<F, R, E>(&mut self, callback: F) -> StdResult<R, E>
    where
        F: AsyncFnOnce(&mut Transaction<&mut Self>) -> StdResult<R, E>,
        Self: Sized,
        E: From<Error>,
    {
        let mut transaction = self.begin().await?;
        match callback(&mut transaction).await {
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

    /// Compile `sql` to validate it and warm the statement cache.
    ///
    /// This does not execute the statement.
    pub fn prepare<'c, 'q: 'c>(&'c self, sql: &'q str) -> BoxFuture<'c, Result<()>> {
        Box::pin(async move { self.worker.prepare(sql).await })
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

impl Drop for ConnectionState {
    fn drop(&mut self) {
        // explicitly drop statements before the connection handle is dropped
        self.statements.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::Connection;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn connection_is_send_sync() {
        assert_send_sync::<Connection>();
    }
}
