use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
};

use either::Either;
use tokio::sync::{Mutex, oneshot};

use crate::{
    QueryResult, Row,
    error::{Error, Result},
    sqlite::{
        Arguments,
        connection::{ConnectionState, establish::EstablishParams, execute},
        statement::Statement,
    },
    transaction::{
        begin_ansi_transaction_sql, commit_ansi_transaction_sql, rollback_ansi_transaction_sql,
    },
};

// Each SQLite connection has a dedicated thread. It's possible to create a worker pool for this,
// but given typical application usage patterns for SQLite, the simplicity of a single-threaded
// worker is preferred.

/// Background worker thread driving a SQLite connection.
pub struct ConnectionWorker {
    /// Command channel to the worker thread.
    command_tx: flume::Sender<Command>,
    /// Mutex for locking access to the database.
    pub(crate) shared: Arc<WorkerSharedState>,
    /// Join handle for the worker thread.
    join_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

// ConnectionWorker is safe to share between threads because:
// - command_tx is Sync (flume::Sender implements Sync)
// - shared is Arc<WorkerSharedState> which is Sync
// - join_handle is Arc<Mutex<>> which is Sync
unsafe impl Sync for ConnectionWorker {}

/// Shared state between async tasks and the worker thread.
pub struct WorkerSharedState {
    /// Cached statement size tracking.
    pub(crate) cached_statements_size: AtomicUsize,
    /// Mutex-protected connection state.
    pub(crate) conn: Mutex<ConnectionState>,
}

#[allow(dead_code)]
/// Commands sent to the worker thread.
enum Command {
    /// Prepare a statement and return it.
    Prepare {
        /// SQL text to prepare.
        query: Box<str>,
        /// Response channel.
        tx: oneshot::Sender<Result<Statement>>,
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
        /// Response channel.
        tx: rendezvous_oneshot::Sender<Result<()>>,
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

impl ConnectionWorker {
    /// Spawn a worker thread and establish the SQLite connection.
    pub(crate) async fn establish(params: EstablishParams) -> Result<Self> {
        let (establish_tx, establish_rx) = oneshot::channel();

        let join_handle = thread::Builder::new()
            .name(params.thread_name.clone())
            .spawn(move || {
                let (command_tx, command_rx) = flume::bounded(params.command_channel_size);

                let conn = match params.establish() {
                    Ok(conn) => conn,
                    Err(e) => {
                        establish_tx.send(Err(e)).ok();
                        return;
                    }
                };

                let shared = Arc::new(WorkerSharedState {
                    cached_statements_size: AtomicUsize::new(0),
                    // note: this mutex is used to synchronize access to the
                    // database from both the worker thread and async tasks.
                    // tokio's mutex is fair so we do not need any additional
                    // configuration here.
                    conn: Mutex::new(conn),
                });
                let mut conn = match shared.conn.try_lock() {
                    Ok(lock) => lock,
                    Err(e) => {
                        establish_tx.send(Err(e.into())).ok();
                        return;
                    }
                };

                if establish_tx
                    .send(Ok((command_tx, Arc::clone(&shared))))
                    .is_err()
                {
                    return;
                }

                // If COMMIT or ROLLBACK is processed but not acknowledged, there would be another
                // ROLLBACK sent when the `Transaction` drops. We need to ignore it otherwise we
                // would rollback an already completed transaction.
                let mut ignore_next_start_rollback = false;

                for cmd in command_rx {
                    match cmd {
                        Command::Prepare { query, tx } => {
                            tx.send(prepare(&mut conn, &query).inspect(|_prepared| {
                                update_cached_statements_size(
                                    &conn,
                                    &shared.cached_statements_size,
                                );
                            }))
                            .ok();
                        }
                        Command::Execute {
                            query,
                            arguments,
                            tx,
                        } => {
                            let iter = match execute::iter(&mut conn, &query, arguments)
                            {
                                Ok(iter) => iter,
                                Err(e) => {
                                    tx.send(Err(e)).ok();
                                    continue;
                                }
                            };

                            for res in iter {
                                if tx.send(res).is_err() {
                                    break;
                                }
                            }

                            update_cached_statements_size(&conn, &shared.cached_statements_size);
                        }
                        Command::Begin { tx } => {
                            let depth = conn.transaction_depth;
                            let res =
                                conn.handle
                                    .exec(begin_ansi_transaction_sql(depth))
                                    .map(|_| {
                                        conn.transaction_depth += 1;
                                    });
                            let res_ok = res.is_ok();

                            if tx.blocking_send(res).is_err() && res_ok {
                                // The BEGIN was processed but not acknowledged. This means no
                                // `Transaction` was created and so there is no way to commit /
                                // rollback this transaction. We need to roll it back
                                // immediately otherwise it would remain started forever.
                                if let Err(error) = conn
                                    .handle
                                    .exec(rollback_ansi_transaction_sql(depth + 1))
                                    .map(|_| {
                                        conn.transaction_depth -= 1;
                                    })
                                {
                                    // The rollback failed. To prevent leaving the connection
                                    // in an inconsistent state we shutdown this worker which
                                    // causes any subsequent operation on the connection to fail.
                                    tracing::error!(%error, "failed to rollback cancelled transaction");
                                    break;
                                }
                            }
                        }
                        Command::Commit { tx } => {
                            let depth = conn.transaction_depth;

                            let res = if depth > 0 {
                                conn.handle
                                    .exec(commit_ansi_transaction_sql(depth))
                                    .map(|_| {
                                        conn.transaction_depth -= 1;
                                    })
                            } else {
                                Ok(())
                            };
                            let res_ok = res.is_ok();

                            if tx.blocking_send(res).is_err() && res_ok {
                                // The COMMIT was processed but not acknowledged. This means that
                                // the `Transaction` doesn't know it was committed and will try to
                                // rollback on drop. We need to ignore that rollback.
                                ignore_next_start_rollback = true;
                            }
                        }
                        Command::Rollback { tx } => {
                            if ignore_next_start_rollback && tx.is_none() {
                                ignore_next_start_rollback = false;
                                continue;
                            }

                            let depth = conn.transaction_depth;

                            let res = if depth > 0 {
                                conn.handle
                                    .exec(rollback_ansi_transaction_sql(depth))
                                    .map(|_| {
                                        conn.transaction_depth -= 1;
                                    })
                            } else {
                                Ok(())
                            };

                            let res_ok = res.is_ok();

                            if let Some(tx) = tx && tx.blocking_send(res).is_err() && res_ok {
                                // The ROLLBACK was processed but not acknowledged. This means
                                // that the `Transaction` doesn't know it was rolled back and
                                // will try to rollback again on drop. We need to ignore that
                                // rollback.
                                ignore_next_start_rollback = true;
                            }
                        }

                        #[cfg(test)]
                        Command::ClearCache { tx } => {
                            conn.statements.clear();
                            update_cached_statements_size(&conn, &shared.cached_statements_size);
                            tx.send(()).ok();
                        }

                        Command::Shutdown { tx } => {
                            conn.statements.clear();
                            let res = conn.handle.close();

                            // drop the connection references before sending confirmation
                            // and ending the command loop
                            drop(conn);
                            drop(shared);
                            let _send_result = tx.send(res);
                            return;
                        }
                    }
                }
            })?;

        let (command_tx, shared) = establish_rx.await.map_err(|_| Error::WorkerCrashed)??;

        Ok(Self {
            command_tx,
            shared,
            join_handle: Arc::new(Mutex::new(Some(join_handle))),
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
    pub(crate) async fn prepare(&self, query: &str) -> Result<Statement> {
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
    pub(crate) async fn begin(&self) -> Result<()> {
        self.oneshot_cmd_with_ack(|tx| Command::Begin { tx })
            .await?
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
fn prepare(conn: &mut ConnectionState, query: &str) -> Result<Statement> {
    // prepare statement object (or checkout from cache)
    let statement = conn.statements.get(query)?;

    while let Some(_statement) = statement.prepare_next(&conn.handle)? {
        // prepare all statements in the compound query
    }

    Ok(Statement {
        sql: query.to_string(),
    })
}

/// Update the cached statement size metric.
fn update_cached_statements_size(conn: &ConnectionState, size: &AtomicUsize) {
    size.store(conn.statements.len(), Ordering::Release);
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
