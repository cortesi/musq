use std::{
    cell::Cell,
    env,
    ffi::{CStr, CString, c_void},
    io,
    os::raw::c_int,
    path::{Path, PathBuf},
    ptr::{self, NonNull},
    slice,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use either::Either;
use libsqlite3_sys::{sqlite3, sqlite3_backup};
use tokio::sync::{Mutex as TokioMutex, oneshot};

use crate::{
    QueryResult, Row,
    error::{Error, PrimaryErrCode, Result},
    sqlite::{
        Arguments,
        connection::{
            BackupReport, ConnectionState, DbStatus, DbStatusKind, DeserializeMode, WalCheckpoint,
            WalCheckpointMode, establish::EstablishParams, execute,
        },
        error::SqliteError,
        ffi,
    },
    transaction::{TransactionBehavior, TxnState, begin_sql, commit_sql, rollback_sql},
};

/// Number of VM opcodes between progress-handler callbacks.
const PROGRESS_INTERVAL: i32 = 1000;

// Each SQLite connection has a dedicated thread. It's possible to create a worker pool for this,
// but given typical application usage patterns for SQLite, the simplicity of a single-threaded
// worker is preferred.

/// Background worker thread driving a SQLite connection.
pub struct ConnectionWorker {
    /// Command channel to the worker thread.
    command_tx: flume::Sender<Command>,
    /// Shared cancellation and depth state.
    pub(crate) shared: Arc<WorkerSharedState>,
    /// Join handle for the worker thread.
    join_handle: Arc<TokioMutex<Option<JoinHandle<()>>>>,
}

/// Shared state between async tasks and the worker thread.
pub struct WorkerSharedState {
    /// Cached statement size tracking.
    pub(crate) cached_statements_size: AtomicUsize,
    /// Nested transaction depth maintained by the worker.
    pub(crate) transaction_depth: AtomicUsize,
    /// Live SQLite handle for [`sqlite3_interrupt`], or `None` after close.
    db: Mutex<Option<PublishedDb>>,
}

/// Raw SQLite handle published for [`sqlite3_interrupt`].
///
/// The worker stores `Some` after open and `None` before `sqlite3_close`.
/// [`WorkerSharedState::interrupt`] takes the mutex and calls
/// `sqlite3_interrupt` only while the value is `Some`.
#[derive(Clone, Copy)]
struct PublishedDb(NonNull<sqlite3>);

// SAFETY: the pointer is only used to call `sqlite3_interrupt` while the
// publishing mutex is held, and only when the worker has not closed the handle.
unsafe impl Send for PublishedDb {}
unsafe impl Sync for PublishedDb {}

/// Clears the published SQLite pointer before the worker drops the handle.
struct ClearDbOnDrop {
    /// Shared state that publishes the handle.
    shared: Arc<WorkerSharedState>,
}

impl Drop for ClearDbOnDrop {
    fn drop(&mut self) {
        *self
            .shared
            .db
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// SQLite allocation that is freed with [`ffi::free`].
struct SqliteAlloc(*mut u8);

impl Drop for SqliteAlloc {
    fn drop(&mut self) {
        unsafe { ffi::free(self.0.cast()) }
    }
}

/// Closes a destination SQLite handle opened for backup.
struct DestGuard(*mut sqlite3);

impl Drop for DestGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { ffi::close(self.0) }.ok();
        }
    }
}

/// Finishes an online backup handle.
struct BackupFinish(*mut sqlite3_backup);

impl Drop for BackupFinish {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { ffi::backup_finish(self.0) };
            self.0 = ptr::null_mut();
        }
    }
}

/// A cloneable handle that interrupts a running statement.
///
/// Call [`interrupt`](Self::interrupt) from another task while a query runs
/// on the connection. A call after the connection has closed is a no-op.
#[derive(Clone)]
pub struct InterruptHandle {
    /// Shared worker state that publishes the SQLite handle.
    shared: Arc<WorkerSharedState>,
}

impl InterruptHandle {
    /// Interrupt the statement currently running on the connection.
    ///
    /// The in-flight statement fails with `SQLITE_INTERRUPT`. If that
    /// statement was a write inside an explicit transaction, SQLite rolls
    /// the transaction back. The next `commit` or `rollback` then returns
    /// [`Error::TransactionAborted`]. Later statements run normally.
    pub fn interrupt(&self) {
        self.shared.interrupt();
    }
}

impl WorkerSharedState {
    /// Interrupt the published handle, if the worker has not closed it.
    fn interrupt(&self) {
        let db = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(handle) = *db {
            // SAFETY: the mutex is held and the pointer is `Some`, so the
            // worker has not closed this connection.
            unsafe { ffi::interrupt(handle.0.as_ptr()) }
        }
    }
}

/// Per-statement deadline enforced by [`sqlite3_progress_handler`].
struct StatementTimeout {
    /// Maximum time a statement may run.
    duration: Duration,
    /// Deadline for the statement that is currently running.
    deadline: Cell<Option<Instant>>,
}

/// Clears the progress-handler deadline when a command finishes.
struct TimeoutGuard<'a> {
    /// Timeout state that owns the deadline cell.
    timeout: &'a StatementTimeout,
}

impl Drop for TimeoutGuard<'_> {
    fn drop(&mut self) {
        self.timeout.deadline.set(None);
    }
}

/// Arm the progress-handler deadline for one command.
fn arm_timeout(timeout: Option<&StatementTimeout>) -> Option<TimeoutGuard<'_>> {
    let timeout = timeout?;
    timeout
        .deadline
        .set(Some(Instant::now() + timeout.duration));
    Some(TimeoutGuard { timeout })
}

/// SQLite progress-handler callback. Returns non-zero when the deadline has passed.
unsafe extern "C" fn progress_callback(p_arg: *mut c_void) -> c_int {
    // SAFETY: the worker registers this pointer as `&Cell<Option<Instant>>`
    // and keeps that cell alive until the connection is closed.
    let deadline = unsafe { &*p_arg.cast::<Cell<Option<Instant>>>() };
    c_int::from(matches!(deadline.get(), Some(deadline) if Instant::now() >= deadline))
}

#[allow(dead_code)]
/// Commands sent to the worker thread.
enum Command {
    /// Prepare a statement and return it.
    Prepare {
        /// SQL text to prepare.
        query: Box<str>,
        /// Response channel.
        tx: oneshot::Sender<Result<()>>,
    },
    /// Execute a statement and stream results.
    Execute {
        /// SQL text to execute.
        query: Box<str>,
        /// Optional arguments to bind.
        arguments: Option<Arguments>,
        /// Result channel.
        tx: flume::Sender<Result<Either<QueryResult, Row>>>,
    },
    /// Begin a transaction.
    Begin {
        /// Start mode for a top-level transaction.
        behavior: TransactionBehavior,
        /// Response channel.
        tx: rendezvous_oneshot::Sender<Result<()>>,
    },
    /// Report whether SQLite is in autocommit mode.
    IsAutocommit {
        /// Response channel.
        tx: oneshot::Sender<Result<bool>>,
    },
    /// Report the SQLite transaction lock state.
    TransactionState {
        /// Response channel.
        tx: oneshot::Sender<Result<TxnState>>,
    },
    /// Serialize a schema to a byte image.
    Serialize {
        /// Schema name, such as `main`.
        schema: CString,
        /// Response channel.
        tx: oneshot::Sender<Result<Vec<u8>>>,
    },
    /// Copy the source database to a destination file.
    BackupToPath {
        /// Destination file as a C string.
        dest: CString,
        /// Destination path for same-file comparison.
        dest_path: PathBuf,
        /// Pages to copy per `sqlite3_backup_step`. Zero copies all remaining.
        pages_per_step: i32,
        /// Response channel.
        tx: oneshot::Sender<Result<BackupReport>>,
    },
    /// Replace a schema from a byte image.
    Deserialize {
        /// Schema name, such as `main`.
        schema: CString,
        /// Database image.
        bytes: Vec<u8>,
        /// How SQLite should treat the buffer.
        mode: DeserializeMode,
        /// Response channel.
        tx: oneshot::Sender<Result<()>>,
    },
    /// Commit a transaction.
    Commit {
        /// Response channel.
        tx: rendezvous_oneshot::Sender<Result<()>>,
    },
    /// Roll back a transaction.
    Rollback {
        /// Optional response channel.
        tx: Option<rendezvous_oneshot::Sender<Result<()>>>,
    },
    /// Return a database status counter.
    DbStatus {
        /// Status counter kind.
        kind: DbStatusKind,
        /// Whether SQLite should reset the high-water mark after reading it.
        reset_highwater: bool,
        /// Response channel.
        tx: oneshot::Sender<Result<DbStatus>>,
    },
    /// Run a WAL checkpoint operation.
    WalCheckpoint {
        /// Optional attached database schema name.
        schema: Option<CString>,
        /// Checkpoint mode.
        mode: WalCheckpointMode,
        /// Response channel.
        tx: oneshot::Sender<Result<WalCheckpoint>>,
    },
    /// Return the parser stack depth limit.
    ParserDepthLimit {
        /// Response channel.
        tx: oneshot::Sender<Result<u32>>,
    },

    #[cfg(test)]
    /// Clear cached statements (tests only).
    ClearCache {
        /// Response channel.
        tx: oneshot::Sender<()>,
    },

    /// Shut down the worker thread.
    Shutdown {
        /// Response channel.
        tx: oneshot::Sender<Result<()>>,
    },
}

/// Per-connection state owned by the worker thread.
struct WorkerSession {
    /// SQLite connection owned by this worker.
    conn: ConnectionState,
    /// Shared interrupt and depth state.
    shared: Arc<WorkerSharedState>,
    /// Optional per-statement timeout.
    timeout: Option<Box<StatementTimeout>>,
    /// Skip the next drop-triggered rollback after a lost commit or rollback ack.
    ignore_next_start_rollback: bool,
    /// Set when an interrupt rolled back an explicit transaction.
    transaction_aborted: bool,
    /// Clears the published handle on every worker exit path.
    _clear_db: ClearDbOnDrop,
}

impl WorkerSession {
    /// Open SQLite, publish the interrupt handle, and report the command channel.
    fn start(
        params: &EstablishParams,
        command_tx: flume::Sender<Command>,
        establish_tx: oneshot::Sender<Result<(flume::Sender<Command>, Arc<WorkerSharedState>)>>,
    ) -> Option<Self> {
        let conn = match params.establish() {
            Ok(conn) => conn,
            Err(e) => {
                establish_tx.send(Err(e)).ok();
                return None;
            }
        };

        let timeout = params.statement_timeout.map(|duration| {
            Box::new(StatementTimeout {
                duration,
                deadline: Cell::new(None),
            })
        });
        if let Some(timeout) = timeout.as_ref() {
            // SAFETY: the handle is live and `timeout` lives until this
            // worker returns, after which the connection is closed.
            unsafe {
                ffi::progress_handler(
                    conn.handle.as_ptr(),
                    PROGRESS_INTERVAL,
                    Some(progress_callback),
                    ptr::from_ref(&timeout.deadline).cast::<c_void>().cast_mut(),
                );
            }
        }

        let shared = Arc::new(WorkerSharedState {
            cached_statements_size: AtomicUsize::new(0),
            transaction_depth: AtomicUsize::new(0),
            db: Mutex::new(None),
        });
        let clear_db = ClearDbOnDrop {
            shared: Arc::clone(&shared),
        };
        {
            let mut db = shared.db.lock().unwrap_or_else(PoisonError::into_inner);
            *db = Some(PublishedDb(conn.handle.as_non_null()));
        }

        if establish_tx
            .send(Ok((command_tx, Arc::clone(&shared))))
            .is_err()
        {
            return None;
        }

        Some(Self {
            conn,
            shared,
            timeout,
            ignore_next_start_rollback: false,
            transaction_aborted: false,
            _clear_db: clear_db,
        })
    }

    /// Handle one command. Returns `true` when the worker should stop.
    fn handle(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::Prepare { query, tx } => self.prepare(&query, tx),
            Command::Execute {
                query,
                arguments,
                tx,
            } => self.execute(&query, arguments, &tx),
            Command::Begin { behavior, tx } => return self.begin(behavior, tx),
            Command::IsAutocommit { tx } => {
                tx.send(Ok(unsafe {
                    ffi::get_autocommit(self.conn.handle.as_ptr())
                }))
                .ok();
            }
            Command::TransactionState { tx } => {
                tx.send(txn_state(&self.conn)).ok();
            }
            Command::Serialize { schema, tx } => {
                tx.send(serialize_schema(&self.conn, &schema)).ok();
            }
            Command::Deserialize {
                schema,
                bytes,
                mode,
                tx,
            } => {
                tx.send(deserialize_schema(&mut self.conn, &schema, &bytes, mode))
                    .ok();
            }
            Command::BackupToPath {
                dest,
                dest_path,
                pages_per_step,
                tx,
            } => {
                tx.send(backup_to_path(
                    &self.conn,
                    &dest,
                    &dest_path,
                    pages_per_step,
                ))
                .ok();
            }
            Command::Commit { tx } => self.commit(tx),
            Command::Rollback { tx } => self.rollback(tx),
            Command::DbStatus {
                kind,
                reset_highwater,
                tx,
            } => {
                tx.send(db_status(&self.conn, kind, reset_highwater)).ok();
            }
            Command::WalCheckpoint { schema, mode, tx } => {
                let _timeout = arm_timeout(self.timeout.as_deref());
                tx.send(wal_checkpoint(&self.conn, schema.as_ref(), mode))
                    .ok();
            }
            Command::ParserDepthLimit { tx } => {
                tx.send(parser_depth_limit(&self.conn)).ok();
            }
            #[cfg(test)]
            Command::ClearCache { tx } => {
                self.conn.statements.clear();
                update_cached_statements_size(&self.conn, &self.shared.cached_statements_size);
                tx.send(()).ok();
            }
            Command::Shutdown { tx } => {
                self.shutdown(tx);
                return true;
            }
        }
        false
    }

    /// Compile `query` and warm the statement cache.
    fn prepare(&mut self, query: &str, tx: oneshot::Sender<Result<()>>) {
        let _timeout = arm_timeout(self.timeout.as_deref());
        let res = prepare(&mut self.conn, query).inspect(|_prepared| {
            update_cached_statements_size(&self.conn, &self.shared.cached_statements_size);
        });
        if let Err(ref e) = res {
            reconcile_interrupt(
                &mut self.conn,
                &self.shared,
                e,
                &mut self.transaction_aborted,
            );
        }
        tx.send(res).ok();
    }

    /// Execute `query` and stream rows until completion, interrupt, or cancel.
    fn execute(
        &mut self,
        query: &str,
        arguments: Option<Arguments>,
        tx: &flume::Sender<Result<Either<QueryResult, Row>>>,
    ) {
        let _timeout = arm_timeout(self.timeout.as_deref());
        if stream_rows(&mut self.conn, query, arguments, tx) {
            reconcile_after_interrupt(&mut self.conn, &self.shared, &mut self.transaction_aborted);
        }
        update_cached_statements_size(&self.conn, &self.shared.cached_statements_size);
    }

    /// Begin a transaction or savepoint. Returns `true` if the worker should stop.
    fn begin(
        &mut self,
        behavior: TransactionBehavior,
        tx: rendezvous_oneshot::Sender<Result<()>>,
    ) -> bool {
        self.transaction_aborted = false;
        let _timeout = arm_timeout(self.timeout.as_deref());
        let depth = self.conn.transaction_depth;
        let res = self.conn.handle.exec(begin_sql(depth, behavior)).map(|_| {
            set_transaction_depth(&mut self.conn, &self.shared, depth + 1);
        });
        if let Err(ref e) = res {
            reconcile_interrupt(
                &mut self.conn,
                &self.shared,
                e,
                &mut self.transaction_aborted,
            );
        }
        let res_ok = res.is_ok();

        if tx.blocking_send(res).is_err() && res_ok {
            let depth = self.conn.transaction_depth;
            if let Err(error) = self.conn.handle.exec(rollback_sql(depth)).map(|_| {
                set_transaction_depth(&mut self.conn, &self.shared, depth - 1);
            }) {
                tracing::error!(%error, "failed to rollback cancelled transaction");
                return true;
            }
        }
        false
    }

    /// Commit the current transaction or savepoint.
    fn commit(&mut self, tx: rendezvous_oneshot::Sender<Result<()>>) {
        if self.transaction_aborted {
            tx.blocking_send(Err(Error::TransactionAborted)).ok();
            return;
        }

        debug_assert_depth_matches_autocommit(&self.conn);
        let _timeout = arm_timeout(self.timeout.as_deref());
        let depth = self.conn.transaction_depth;
        let res = if depth > 0 {
            self.conn.handle.exec(commit_sql(depth)).map(|_| {
                set_transaction_depth(&mut self.conn, &self.shared, depth - 1);
            })
        } else {
            Ok(())
        };
        if let Err(ref e) = res {
            reconcile_interrupt(
                &mut self.conn,
                &self.shared,
                e,
                &mut self.transaction_aborted,
            );
        } else {
            debug_assert_depth_matches_autocommit(&self.conn);
        }
        let res_ok = res.is_ok();
        if tx.blocking_send(res).is_err() && res_ok {
            self.ignore_next_start_rollback = true;
        }
    }

    /// Roll back the current transaction or savepoint.
    fn rollback(&mut self, tx: Option<rendezvous_oneshot::Sender<Result<()>>>) {
        if self.ignore_next_start_rollback && tx.is_none() {
            self.ignore_next_start_rollback = false;
            return;
        }

        if self.transaction_aborted {
            if let Some(tx) = tx {
                tx.blocking_send(Err(Error::TransactionAborted)).ok();
            }
            return;
        }

        debug_assert_depth_matches_autocommit(&self.conn);
        let _timeout = arm_timeout(self.timeout.as_deref());
        let depth = self.conn.transaction_depth;
        let res = if depth > 0 {
            self.conn.handle.exec(rollback_sql(depth)).map(|_| {
                set_transaction_depth(&mut self.conn, &self.shared, depth - 1);
            })
        } else {
            Ok(())
        };
        if let Err(ref e) = res {
            reconcile_interrupt(
                &mut self.conn,
                &self.shared,
                e,
                &mut self.transaction_aborted,
            );
        } else {
            debug_assert_depth_matches_autocommit(&self.conn);
        }
        let res_ok = res.is_ok();
        if let Some(tx) = tx
            && tx.blocking_send(res).is_err()
            && res_ok
        {
            self.ignore_next_start_rollback = true;
        }
    }

    /// Close the SQLite handle after clearing the published interrupt pointer.
    fn shutdown(&mut self, tx: oneshot::Sender<Result<()>>) {
        self.conn.statements.clear();
        *self
            .shared
            .db
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        let res = self.conn.handle.close();
        tx.send(res).ok();
    }
}

/// Stream query results. Returns `true` when SQLite reported `SQLITE_INTERRUPT`.
fn stream_rows(
    conn: &mut ConnectionState,
    query: &str,
    arguments: Option<Arguments>,
    tx: &flume::Sender<Result<Either<QueryResult, Row>>>,
) -> bool {
    let iter = match execute::iter(conn, query, arguments) {
        Ok(iter) => iter,
        Err(e) => {
            let interrupted = is_interrupt(&e);
            tx.send(Err(e)).ok();
            return interrupted;
        }
    };
    let mut interrupted = false;
    for res in iter {
        let this_interrupt = res.as_ref().err().is_some_and(is_interrupt);
        interrupted |= this_interrupt;
        if tx.send(res).is_err() || this_interrupt {
            break;
        }
    }
    interrupted
}

impl ConnectionWorker {
    /// Spawn a worker thread and establish the SQLite connection.
    pub(crate) async fn establish(params: EstablishParams) -> Result<Self> {
        let (establish_tx, establish_rx) = oneshot::channel();

        let join_handle = thread::Builder::new()
            .name(params.thread_name.clone())
            .spawn(move || {
                let (command_tx, command_rx) = flume::bounded(params.command_channel_size);
                let Some(mut session) = WorkerSession::start(&params, command_tx, establish_tx)
                else {
                    return;
                };
                for cmd in command_rx {
                    if session.handle(cmd) {
                        return;
                    }
                }
            })?;

        let (command_tx, shared) = establish_rx.await.map_err(|_| Error::WorkerCrashed)??;

        Ok(Self {
            command_tx,
            shared,
            join_handle: Arc::new(TokioMutex::new(Some(join_handle))),
        })
    }

    #[allow(dead_code)]
    /// Returns whether the worker has been shut down.
    pub(crate) fn is_shutdown(&self) -> bool {
        // For now, just return false as checking would require async
        // This is only used in drop, so it's not critical
        false
    }

    #[allow(dead_code)]
    /// Prepare a SQL statement on the worker thread.
    pub(crate) async fn prepare(&self, query: &str) -> Result<()> {
        self.oneshot_cmd(|tx| Command::Prepare {
            query: query.into(),
            tx,
        })
        .await?
    }

    /// Execute a SQL statement and stream the results.
    ///
    /// We take an owned string here - we immediatley copy it into the command anyway.
    pub(crate) async fn execute(
        &self,
        query: String,
        args: Option<Arguments>,
        chan_size: usize,
    ) -> Result<flume::Receiver<Result<Either<QueryResult, Row>>>> {
        let (tx, rx) = flume::bounded(chan_size);

        self.command_tx
            .send_async(Command::Execute {
                query: query.into(),
                arguments: args,
                tx,
            })
            .await
            .map_err(|_| Error::WorkerCrashed)?;

        Ok(rx)
    }

    /// Begin a transaction on the worker thread.
    pub(crate) async fn begin(&self, behavior: TransactionBehavior) -> Result<()> {
        self.oneshot_cmd_with_ack(|tx| Command::Begin { behavior, tx })
            .await?
    }

    /// Report whether SQLite is in autocommit mode.
    pub(crate) async fn is_autocommit(&self) -> Result<bool> {
        self.oneshot_cmd(|tx| Command::IsAutocommit { tx }).await?
    }

    /// Report the SQLite transaction lock state.
    pub(crate) async fn transaction_state(&self) -> Result<TxnState> {
        self.oneshot_cmd(|tx| Command::TransactionState { tx })
            .await?
    }

    /// Serialize `schema` to a SQLite database image.
    pub(crate) async fn serialize(&self, schema: CString) -> Result<Vec<u8>> {
        self.oneshot_cmd(|tx| Command::Serialize { schema, tx })
            .await?
    }

    /// Replace `schema` from a SQLite database image.
    pub(crate) async fn deserialize(
        &self,
        schema: CString,
        bytes: Vec<u8>,
        mode: DeserializeMode,
    ) -> Result<()> {
        self.oneshot_cmd(|tx| Command::Deserialize {
            schema,
            bytes,
            mode,
            tx,
        })
        .await?
    }

    /// Copy this database to `dest` with `sqlite3_backup`.
    pub(crate) async fn backup_to_path(
        &self,
        dest: CString,
        dest_path: PathBuf,
        pages_per_step: i32,
    ) -> Result<BackupReport> {
        self.oneshot_cmd(|tx| Command::BackupToPath {
            dest,
            dest_path,
            pages_per_step,
            tx,
        })
        .await?
    }

    /// Interrupt the statement currently running on this connection.
    pub(crate) fn interrupt(&self) {
        self.shared.interrupt();
    }

    /// Return a cloneable handle that can interrupt this connection.
    pub(crate) fn interrupt_handle(&self) -> InterruptHandle {
        InterruptHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Commit the current transaction on the worker thread.
    pub(crate) async fn commit(&self) -> Result<()> {
        self.oneshot_cmd_with_ack(|tx| Command::Commit { tx })
            .await?
    }

    /// Roll back the current transaction on the worker thread.
    pub(crate) async fn rollback(&self) -> Result<()> {
        self.oneshot_cmd_with_ack(|tx| Command::Rollback { tx: Some(tx) })
            .await?
    }

    /// Start an asynchronous rollback without awaiting acknowledgement.
    pub(crate) fn start_rollback(&self) -> Result<()> {
        self.command_tx
            .send(Command::Rollback { tx: None })
            .map_err(|_| Error::WorkerCrashed)
    }

    /// Return a database status counter from the worker thread.
    pub(crate) async fn db_status(
        &self,
        kind: DbStatusKind,
        reset_highwater: bool,
    ) -> Result<DbStatus> {
        self.oneshot_cmd(|tx| Command::DbStatus {
            kind,
            reset_highwater,
            tx,
        })
        .await?
    }

    /// Run a WAL checkpoint operation on the worker thread.
    pub(crate) async fn wal_checkpoint(
        &self,
        schema: Option<CString>,
        mode: WalCheckpointMode,
    ) -> Result<WalCheckpoint> {
        self.oneshot_cmd(|tx| Command::WalCheckpoint { schema, mode, tx })
            .await?
    }

    /// Return the parser stack depth limit from the worker thread.
    pub(crate) async fn parser_depth_limit(&self) -> Result<u32> {
        self.oneshot_cmd(|tx| Command::ParserDepthLimit { tx })
            .await?
    }

    #[allow(dead_code)]
    /// Send a oneshot command and await the response.
    async fn oneshot_cmd<F, T>(&self, command: F) -> Result<T>
    where
        F: FnOnce(oneshot::Sender<T>) -> Command,
    {
        let (tx, rx) = oneshot::channel();

        self.command_tx
            .send_async(command(tx))
            .await
            .map_err(|_| Error::WorkerCrashed)?;

        rx.await.map_err(|_| Error::WorkerCrashed)
    }

    /// Send a oneshot command requiring acknowledgement before returning.
    async fn oneshot_cmd_with_ack<F, T>(&self, command: F) -> Result<T>
    where
        F: FnOnce(rendezvous_oneshot::Sender<T>) -> Command,
    {
        let (tx, rx) = rendezvous_oneshot::channel();

        self.command_tx
            .send_async(command(tx))
            .await
            .map_err(|_| Error::WorkerCrashed)?;

        rx.recv().await.map_err(|_| Error::WorkerCrashed)
    }

    #[cfg(test)]
    /// Clear cached statements in tests.
    pub(crate) async fn clear_cache(&self) -> Result<()> {
        self.oneshot_cmd(|tx| Command::ClearCache { tx }).await
    }

    /// Send a command to the worker to shut down the processing thread.
    ///
    /// A `WorkerCrashed` error may be returned if the thread has already stopped.
    pub(crate) async fn shutdown(&self) -> Result<()> {
        let join_handle = self.join_handle.lock().await.take();
        let (tx, rx) = oneshot::channel();

        let send_res = self
            .command_tx
            .send(Command::Shutdown { tx })
            .map_err(|_| Error::WorkerCrashed);

        if let Err(e) = send_res {
            if let Some(handle) = join_handle {
                let _join_result = handle.join();
            }
            return Err(e);
        }

        // wait for the response
        let res = rx.await.map_err(|_| Error::WorkerCrashed)?;
        res?;

        if let Some(handle) = join_handle {
            handle.join().map_err(|_| Error::WorkerCrashed)?;
        }

        Ok(())
    }
}

/// Prepare a SQL statement, using the cache when possible.
fn prepare(conn: &mut ConnectionState, query: &str) -> Result<()> {
    // prepare statement object (or checkout from cache)
    let statement = conn.statements.get(query)?;

    while let Some(_statement) = statement.prepare_next(&conn.handle)? {
        // prepare all statements in the compound query
    }

    Ok(())
}

/// Update the cached statement size metric.
fn update_cached_statements_size(conn: &ConnectionState, size: &AtomicUsize) {
    size.store(conn.statements.len(), Ordering::Release);
}

/// Record transaction depth on the worker-owned state and the shared atomic.
fn set_transaction_depth(conn: &mut ConnectionState, shared: &WorkerSharedState, depth: usize) {
    conn.transaction_depth = depth;
    shared.transaction_depth.store(depth, Ordering::Release);
}

/// Return whether `err` is `SQLITE_INTERRUPT`.
fn is_interrupt(err: &Error) -> bool {
    matches!(
        err.as_sqlite().map(|error| error.primary),
        Some(PrimaryErrCode::Interrupt)
    )
}

/// Reconcile Musq transaction depth after SQLite interrupts a statement.
fn reconcile_interrupt(
    conn: &mut ConnectionState,
    shared: &WorkerSharedState,
    err: &Error,
    transaction_aborted: &mut bool,
) {
    if is_interrupt(err) {
        reconcile_after_interrupt(conn, shared, transaction_aborted);
    }
}

/// After `SQLITE_INTERRUPT`, reset the depth when SQLite has rolled back.
fn reconcile_after_interrupt(
    conn: &mut ConnectionState,
    shared: &WorkerSharedState,
    transaction_aborted: &mut bool,
) {
    // SAFETY: the worker owns a live connection handle.
    let autocommit = unsafe { ffi::get_autocommit(conn.handle.as_ptr()) };
    if autocommit {
        if conn.transaction_depth > 0 {
            *transaction_aborted = true;
        }
        set_transaction_depth(conn, shared, 0);
    }
}

/// Return a database status counter for the active connection.
fn db_status(
    conn: &ConnectionState,
    kind: DbStatusKind,
    reset_highwater: bool,
) -> Result<DbStatus> {
    let (current, highwater) =
        unsafe { ffi::db_status64(conn.handle.as_ptr(), kind.as_sqlite_code(), reset_highwater) }?;
    Ok(DbStatus { current, highwater })
}

/// Run a WAL checkpoint operation for the active connection.
fn wal_checkpoint(
    conn: &ConnectionState,
    schema: Option<&CString>,
    mode: WalCheckpointMode,
) -> Result<WalCheckpoint> {
    let schema_ptr = schema.map_or(ptr::null(), |schema| schema.as_ptr());
    let (log_frames, checkpointed_frames) =
        unsafe { ffi::wal_checkpoint_v2(conn.handle.as_ptr(), schema_ptr, mode.as_sqlite_code()) }?;
    Ok(WalCheckpoint {
        log_frames: frames_to_option(log_frames),
        checkpointed_frames: frames_to_option(checkpointed_frames),
    })
}

/// Return the parser stack depth limit for the active connection.
fn parser_depth_limit(conn: &ConnectionState) -> Result<u32> {
    let limit = unsafe {
        ffi::limit(
            conn.handle.as_ptr(),
            libsqlite3_sys::SQLITE_LIMIT_PARSER_DEPTH,
            -1,
        )
    };
    u32::try_from(limit).map_err(|_| {
        Error::Protocol(format!(
            "SQLite returned invalid parser depth limit {limit}"
        ))
    })
}

/// Convert SQLite frame counts where `-1` means unavailable.
fn frames_to_option(frames: i32) -> Option<i32> {
    if frames < 0 { None } else { Some(frames) }
}

/// Map `sqlite3_txn_state` to [`TxnState`].
fn txn_state(conn: &ConnectionState) -> Result<TxnState> {
    let state = unsafe { ffi::txn_state(conn.handle.as_ptr(), ptr::null()) };
    match state {
        libsqlite3_sys::SQLITE_TXN_NONE => Ok(TxnState::None),
        libsqlite3_sys::SQLITE_TXN_READ => Ok(TxnState::Read),
        libsqlite3_sys::SQLITE_TXN_WRITE => Ok(TxnState::Write),
        other => Err(Error::Protocol(format!(
            "sqlite3_txn_state returned {other}"
        ))),
    }
}

/// Copy a SQLite schema image into a `Vec` and free the SQLite buffer.
fn serialize_schema(conn: &ConnectionState, schema: &CString) -> Result<Vec<u8>> {
    let (ptr, size) = unsafe { ffi::serialize(conn.handle.as_ptr(), schema.as_ptr()) }?;
    let _alloc = SqliteAlloc(ptr);
    let size = usize::try_from(size)
        .map_err(|_| Error::Protocol(format!("sqlite3_serialize returned invalid size {size}")))?;
    // SAFETY: `ptr` is a SQLite allocation of `size` bytes.
    Ok(unsafe { slice::from_raw_parts(ptr, size) }.to_vec())
}

/// Return whether a database image uses the WAL file format.
fn is_wal_image(bytes: &[u8]) -> bool {
    bytes.len() >= 20 && (bytes[18] == 2 || bytes[19] == 2)
}

/// Load a SQLite schema image, taking ownership of a SQLite-allocated buffer.
fn deserialize_schema(
    conn: &mut ConnectionState,
    schema: &CString,
    bytes: &[u8],
    mode: DeserializeMode,
) -> Result<()> {
    if conn.transaction_depth > 0 || !unsafe { ffi::get_autocommit(conn.handle.as_ptr()) } {
        return Err(Error::Configuration(
            "cannot deserialize while a transaction is open".into(),
        ));
    }
    if is_wal_image(bytes) {
        return Err(Error::Configuration(
            "cannot deserialize a WAL-mode database image".into(),
        ));
    }

    conn.statements.clear();

    let size = i64::try_from(bytes.len())
        .map_err(|_| Error::Configuration("deserialize image is too large".into()))?;
    let ptr = unsafe { ffi::malloc64(bytes.len() as u64) };
    if ptr.is_null() {
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::OutOfMemory,
            "sqlite3_malloc64 failed",
        )));
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.cast::<u8>(), bytes.len());
    }

    let flags = libsqlite3_sys::SQLITE_DESERIALIZE_FREEONCLOSE
        | match mode {
            DeserializeMode::ReadOnly => libsqlite3_sys::SQLITE_DESERIALIZE_READONLY,
            DeserializeMode::Resizable => libsqlite3_sys::SQLITE_DESERIALIZE_RESIZEABLE,
        };

    match unsafe {
        ffi::deserialize(
            conn.handle.as_ptr(),
            schema.as_ptr(),
            ptr.cast::<u8>(),
            size,
            size,
            flags,
        )
    } {
        Ok(()) => Ok(()),
        Err(error) => {
            unsafe { ffi::free(ptr) };
            Err(error.into())
        }
    }
}

/// Return whether `dest` names the same file as the live source database.
fn is_same_sqlite_file(source_filename: &str, dest: &Path) -> bool {
    if source_filename.is_empty() || source_filename == ":memory:" {
        return false;
    }
    let source = Path::new(source_filename);
    if let (Ok(left), Ok(right)) = (source.canonicalize(), dest.canonicalize()) {
        return left == right;
    }
    let dest_abs = if dest.is_absolute() {
        dest.to_path_buf()
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(dest))
            .unwrap_or_else(|_| dest.to_path_buf())
    };
    source == dest_abs || source == dest
}

/// Copy the source database to `dest` using the SQLite backup API.
fn backup_to_path(
    conn: &ConnectionState,
    dest: &CString,
    dest_path: &Path,
    pages_per_step: i32,
) -> Result<BackupReport> {
    let source_name = unsafe { ffi::db_filename(conn.handle.as_ptr(), c"main".as_ptr()) };
    if !source_name.is_null() {
        let source = unsafe { CStr::from_ptr(source_name) }.to_string_lossy();
        if is_same_sqlite_file(&source, dest_path) {
            return Err(Error::Configuration(
                "backup destination path is the same as the source database".into(),
            ));
        }
    }

    let mut dest_db = ptr::null_mut();
    unsafe {
        ffi::open_v2(
            dest.as_ptr(),
            &mut dest_db,
            libsqlite3_sys::SQLITE_OPEN_READWRITE
                | libsqlite3_sys::SQLITE_OPEN_CREATE
                | libsqlite3_sys::SQLITE_OPEN_EXRESCODE,
            ptr::null(),
        )
    }?;
    let dest_guard = DestGuard(dest_db);

    let backup = unsafe {
        ffi::backup_init(
            dest_db,
            c"main".as_ptr(),
            conn.handle.as_ptr(),
            c"main".as_ptr(),
        )
    };
    if backup.is_null() {
        return Err(SqliteError::new(dest_db).into());
    }
    let mut backup_guard = BackupFinish(backup);

    loop {
        let rc = unsafe { ffi::backup_step(backup, pages_per_step) };
        if rc == libsqlite3_sys::SQLITE_DONE {
            break;
        }
        if rc != libsqlite3_sys::SQLITE_OK {
            return Err(SqliteError::new(dest_db).into());
        }
    }

    let pages = unsafe { ffi::backup_pagecount(backup) };
    let remaining = unsafe { ffi::backup_remaining(backup) };
    let finish_rc = unsafe { ffi::backup_finish(backup) };
    backup_guard.0 = ptr::null_mut();
    if finish_rc != libsqlite3_sys::SQLITE_OK {
        return Err(SqliteError::new(dest_db).into());
    }
    drop(dest_guard);
    Ok(BackupReport { pages, remaining })
}

/// Catch a mismatch between Musq depth and SQLite autocommit in tests.
fn debug_assert_depth_matches_autocommit(conn: &ConnectionState) {
    let autocommit = unsafe { ffi::get_autocommit(conn.handle.as_ptr()) };
    debug_assert_eq!(
        conn.transaction_depth == 0,
        autocommit,
        "transaction_depth is {} but sqlite3_get_autocommit is {autocommit}",
        conn.transaction_depth
    );
}

// A oneshot channel where send completes only after the receiver receives the value.
/// Rendezvous-style oneshot channels with acknowledgement.
mod rendezvous_oneshot {
    use std::result::Result as StdResult;

    use super::oneshot;

    /// Error returned when a rendezvous channel is canceled.
    #[derive(Debug)]
    pub struct Canceled;

    /// Create a sender/receiver pair.
    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (inner_tx, inner_rx) = oneshot::channel();
        (Sender { inner: inner_tx }, Receiver { inner: inner_rx })
    }

    /// Sender half for rendezvous delivery.
    pub struct Sender<T> {
        /// Inner channel used for delivery.
        inner: oneshot::Sender<(T, oneshot::Sender<()>)>,
    }

    impl<T> Sender<T> {
        /// Send a value and await acknowledgement.
        pub async fn send(self, value: T) -> StdResult<(), Canceled> {
            let (ack_tx, ack_rx) = oneshot::channel();
            self.inner.send((value, ack_tx)).map_err(|_| Canceled)?;
            ack_rx.await.map_err(|_| Canceled)?;
            Ok(())
        }

        /// Send a value and block until acknowledged.
        pub fn blocking_send(self, value: T) -> StdResult<(), Canceled> {
            futures_executor::block_on(self.send(value))
        }
    }

    /// Receiver half for rendezvous delivery.
    pub struct Receiver<T> {
        /// Inner channel used for delivery.
        inner: oneshot::Receiver<(T, oneshot::Sender<()>)>,
    }

    impl<T> Receiver<T> {
        /// Receive a value and acknowledge receipt.
        pub async fn recv(self) -> StdResult<T, Canceled> {
            let (value, ack_tx) = self.inner.await.map_err(|_| Canceled)?;
            ack_tx.send(()).map_err(|_| Canceled)?;
            Ok(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionWorker, InterruptHandle, WorkerSharedState};

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn connection_worker_is_send_sync() {
        assert_send_sync::<ConnectionWorker>();
        assert_send_sync::<WorkerSharedState>();
        assert_send_sync::<InterruptHandle>();
    }
}
