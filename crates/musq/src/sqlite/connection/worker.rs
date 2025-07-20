use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use tokio::sync::Mutex;
use tokio::sync::oneshot;

use either::Either;

use crate::{
    QueryResult, Row,
    error::{Error, Result},
    sqlite::{
        Arguments, Statement,
        connection::{ConnectionState, establish::EstablishParams, execute},
    },
    transaction::{
        begin_ansi_transaction_sql, commit_ansi_transaction_sql, rollback_ansi_transaction_sql,
    },
};

// Each SQLite connection has a dedicated thread. It's possible to create a worker pool for this,
// but given typical application usage patterns for SQLite, the simplicity of a single-threaded
// worker is preferred.

pub(crate) struct ConnectionWorker {
    command_tx: flume::Sender<Command>,
    /// Mutex for locking access to the database.
    pub(crate) shared: Arc<WorkerSharedState>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

pub(crate) struct WorkerSharedState {
    pub(crate) cached_statements_size: AtomicUsize,
    pub(crate) conn: Mutex<ConnectionState>,
}

enum Command {
    Prepare {
        query: Box<str>,
        tx: oneshot::Sender<Result<Statement>>,
    },
    Execute {
        query: Box<str>,
        arguments: Option<Arguments>,
        tx: flume::Sender<Result<Either<QueryResult, Row>>>,
    },
    Begin {
        tx: rendezvous_oneshot::Sender<Result<()>>,
    },
    Commit {
        tx: rendezvous_oneshot::Sender<Result<()>>,
    },
    Rollback {
        tx: Option<rendezvous_oneshot::Sender<Result<()>>>,
    },
    Shutdown {
        tx: oneshot::Sender<Result<()>>,
    },
}

impl ConnectionWorker {
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
                        Command::Shutdown { tx } => {
                            conn.statements.clear();
                            let res = conn.handle.close();

                            // drop the connection references before sending confirmation
                            // and ending the command loop
                            drop(conn);
                            drop(shared);
                            let _ = tx.send(res);
                            return;
                        }
                    }
                }
            })?;

        let (command_tx, shared) = establish_rx.await.map_err(|_| Error::WorkerCrashed)??;

        Ok(Self {
            command_tx,
            shared,
            join_handle: Some(join_handle),
        })
    }

    pub(crate) fn is_shutdown(&self) -> bool {
        self.join_handle.is_none()
    }

    pub(crate) async fn prepare(&mut self, query: &str) -> Result<Statement> {
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

    pub(crate) async fn begin(&mut self) -> Result<()> {
        self.oneshot_cmd_with_ack(|tx| Command::Begin { tx })
            .await?
    }

    pub(crate) async fn commit(&mut self) -> Result<()> {
        self.oneshot_cmd_with_ack(|tx| Command::Commit { tx })
            .await?
    }

    pub(crate) async fn rollback(&mut self) -> Result<()> {
        self.oneshot_cmd_with_ack(|tx| Command::Rollback { tx: Some(tx) })
            .await?
    }

    pub(crate) fn start_rollback(&mut self) -> Result<()> {
        self.command_tx
            .send(Command::Rollback { tx: None })
            .map_err(|_| Error::WorkerCrashed)
    }

    async fn oneshot_cmd<F, T>(&mut self, command: F) -> Result<T>
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

    async fn oneshot_cmd_with_ack<F, T>(&mut self, command: F) -> Result<T>
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

    /// Send a command to the worker to shut down the processing thread.
    ///
    /// A `WorkerCrashed` error may be returned if the thread has already stopped.
    pub(crate) fn shutdown(&mut self) -> impl Future<Output = Result<()>> {
        let join_handle = self.join_handle.take();
        let (tx, rx) = oneshot::channel();

        let send_res = self
            .command_tx
            .send(Command::Shutdown { tx })
            .map_err(|_| Error::WorkerCrashed);

        async move {
            if let Err(e) = send_res {
                if let Some(handle) = join_handle {
                    let _ = handle.join();
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
}

fn prepare(conn: &mut ConnectionState, query: &str) -> Result<Statement> {
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
    use super::oneshot;

    #[derive(Debug)]
    pub struct Canceled;

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let (inner_tx, inner_rx) = oneshot::channel();
        (Sender { inner: inner_tx }, Receiver { inner: inner_rx })
    }

    pub struct Sender<T> {
        inner: oneshot::Sender<(T, oneshot::Sender<()>)>,
    }

    impl<T> Sender<T> {
        pub async fn send(self, value: T) -> std::result::Result<(), Canceled> {
            let (ack_tx, ack_rx) = oneshot::channel();
            self.inner.send((value, ack_tx)).map_err(|_| Canceled)?;
            ack_rx.await.map_err(|_| Canceled)?;
            Ok(())
        }

        pub fn blocking_send(self, value: T) -> std::result::Result<(), Canceled> {
            futures_executor::block_on(self.send(value))
        }
    }

    pub struct Receiver<T> {
        inner: oneshot::Receiver<(T, oneshot::Sender<()>)>,
    }

    impl<T> Receiver<T> {
        pub async fn recv(self) -> std::result::Result<T, Canceled> {
            let (value, ack_tx) = self.inner.await.map_err(|_| Canceled)?;
            ack_tx.send(()).map_err(|_| Canceled)?;
            Ok(value)
        }
    }
}
