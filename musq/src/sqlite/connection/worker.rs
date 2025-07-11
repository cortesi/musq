use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use futures_channel::oneshot;
use futures_intrusive::sync::{Mutex, MutexGuard};

use crate::{
    Either, QueryResult, Row,
    error::Error,
    sqlite::{
        Arguments, Statement,
        connection::{ConnectionState, establish::EstablishParams, execute},
    },
    transaction::{
        begin_ansi_transaction_sql, commit_ansi_transaction_sql, rollback_ansi_transaction_sql,
    },
};

// Each SQLite connection has a dedicated thread.

// TODO: Tweak this so that we can use a thread pool per pool of SQLite3 connections to reduce
//       OS resource usage. Low priority because a high concurrent load for SQLite3 is very
//       unlikely.

pub(crate) struct ConnectionWorker {
    command_tx: flume::Sender<Command>,
    /// Mutex for locking access to the database.
    pub(crate) shared: Arc<WorkerSharedState>,
}

pub(crate) struct WorkerSharedState {
    pub(crate) cached_statements_size: AtomicUsize,
    pub(crate) conn: Mutex<ConnectionState>,
}

enum Command {
    Prepare {
        query: Box<str>,
        tx: oneshot::Sender<Result<Statement, Error>>,
    },
    Execute {
        query: Box<str>,
        arguments: Option<Arguments>,
        tx: flume::Sender<Result<Either<QueryResult, Row>, Error>>,
    },
    Begin {
        tx: rendezvous_oneshot::Sender<Result<(), Error>>,
    },
    Commit {
        tx: rendezvous_oneshot::Sender<Result<(), Error>>,
    },
    Rollback {
        tx: Option<rendezvous_oneshot::Sender<Result<(), Error>>>,
    },
    UnlockDb,
    ClearCache {
        tx: oneshot::Sender<()>,
    },
    Shutdown {
        tx: oneshot::Sender<()>,
    },
}

impl ConnectionWorker {
    pub(crate) async fn establish(params: EstablishParams) -> Result<Self, Error> {
        let (establish_tx, establish_rx) = oneshot::channel();

        thread::Builder::new()
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
                    // note: must be fair because in `Command::UnlockDb` we unlock the mutex
                    // and then immediately try to relock it; an unfair mutex would immediately
                    // grant us the lock even if another task is waiting.
                    conn: Mutex::new(conn, true),
                });
                let mut conn = shared.conn.try_lock().unwrap();

                if establish_tx
                    .send(Ok(Self {
                        command_tx,
                        shared: Arc::clone(&shared),
                    }))
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

                            if let Some(tx) = tx {
                                if tx.blocking_send(res).is_err() && res_ok {
                                    // The ROLLBACK was processed but not acknowledged. This means
                                    // that the `Transaction` doesn't know it was rolled back and
                                    // will try to rollback again on drop. We need to ignore that
                                    // rollback.
                                    ignore_next_start_rollback = true;
                                }
                            }
                        }
                        Command::ClearCache { tx } => {
                            conn.statements.clear();
                            update_cached_statements_size(&conn, &shared.cached_statements_size);
                            tx.send(()).ok();
                        }
                        Command::UnlockDb => {
                            drop(conn);
                            conn = futures_executor::block_on(shared.conn.lock());
                        }
                        Command::Shutdown { tx } => {
                            // drop the connection references before sending confirmation
                            // and ending the command loop
                            drop(conn);
                            drop(shared);
                            let _ = tx.send(());
                            return;
                        }
                    }
                }
            })?;

        establish_rx.await.map_err(|_| Error::WorkerCrashed)?
    }

    pub(crate) async fn prepare(&mut self, query: &str) -> Result<Statement, Error> {
        self.oneshot_cmd(|tx| Command::Prepare {
            query: query.into(),
            tx,
        })
        .await?
    }

    /// We take an owned string here - we immediatley copy it into the command anyway.
    pub(crate) async fn execute(
        &mut self,
        query: String,
        args: Option<Arguments>,
        chan_size: usize,
    ) -> Result<flume::Receiver<Result<Either<QueryResult, Row>, Error>>, Error> {
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

    pub(crate) async fn begin(&mut self) -> Result<(), Error> {
        self.oneshot_cmd_with_ack(|tx| Command::Begin { tx })
            .await?
    }

    pub(crate) async fn commit(&mut self) -> Result<(), Error> {
        self.oneshot_cmd_with_ack(|tx| Command::Commit { tx })
            .await?
    }

    pub(crate) async fn rollback(&mut self) -> Result<(), Error> {
        self.oneshot_cmd_with_ack(|tx| Command::Rollback { tx: Some(tx) })
            .await?
    }

    pub(crate) fn start_rollback(&mut self) -> Result<(), Error> {
        self.command_tx
            .send(Command::Rollback { tx: None })
            .map_err(|_| Error::WorkerCrashed)
    }

    async fn oneshot_cmd<F, T>(&mut self, command: F) -> Result<T, Error>
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

    async fn oneshot_cmd_with_ack<F, T>(&mut self, command: F) -> Result<T, Error>
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

    pub(crate) async fn clear_cache(&mut self) -> Result<(), Error> {
        self.oneshot_cmd(|tx| Command::ClearCache { tx }).await
    }

    pub(crate) async fn unlock_db(&mut self) -> Result<MutexGuard<'_, ConnectionState>, Error> {
        let (guard, res) = futures_util::future::join(
            // we need to join the wait queue for the lock before we send the message
            self.shared.conn.lock(),
            self.command_tx.send_async(Command::UnlockDb),
        )
        .await;

        res.map_err(|_| Error::WorkerCrashed)?;

        Ok(guard)
    }

    /// Send a command to the worker to shut down the processing thread.
    ///
    /// A `WorkerCrashed` error may be returned if the thread has already stopped.
    pub(crate) fn shutdown(&mut self) -> impl Future<Output = Result<(), Error>> {
        let (tx, rx) = oneshot::channel();

        let send_res = self
            .command_tx
            .send(Command::Shutdown { tx })
            .map_err(|_| Error::WorkerCrashed);

        async move {
            send_res?;

            // wait for the response
            rx.await.map_err(|_| Error::WorkerCrashed)
        }
    }
}

fn prepare(conn: &mut ConnectionState, query: &str) -> Result<Statement, Error> {
    // prepare statement object (or checkout from cache)
    let statement = conn.statements.get(query)?;

    let mut columns = None;

    while let Some(statement) = statement.prepare_next(&mut conn.handle)? {
        // the first non-empty statement is chosen as the statement we pull columns from
        if !statement.columns.is_empty() && columns.is_none() {
            columns = Some(Arc::clone(statement.columns));
        }
    }

    Ok(Statement {
        sql: query.to_string(),
        columns: columns.unwrap_or_default(),
    })
}

fn update_cached_statements_size(conn: &ConnectionState, size: &AtomicUsize) {
    size.store(conn.statements.len(), Ordering::Release);
}

// A oneshot channel where send completes only after the receiver receives the value.
mod rendezvous_oneshot {
    use super::oneshot::{self, Canceled};

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (inner_tx, inner_rx) = oneshot::channel();
        (Sender { inner: inner_tx }, Receiver { inner: inner_rx })
    }

    pub struct Sender<T> {
        inner: oneshot::Sender<(T, oneshot::Sender<()>)>,
    }

    impl<T> Sender<T> {
        pub async fn send(self, value: T) -> Result<(), Canceled> {
            let (ack_tx, ack_rx) = oneshot::channel();
            self.inner.send((value, ack_tx)).map_err(|_| Canceled)?;
            ack_rx.await
        }

        pub fn blocking_send(self, value: T) -> Result<(), Canceled> {
            futures_executor::block_on(self.send(value))
        }
    }

    pub struct Receiver<T> {
        inner: oneshot::Receiver<(T, oneshot::Sender<()>)>,
    }

    impl<T> Receiver<T> {
        pub async fn recv(self) -> Result<T, Canceled> {
            let (value, ack_tx) = self.inner.await?;
            ack_tx.send(()).map_err(|_| Canceled)?;
            Ok(value)
        }
    }
}
