//! Lazy single-threaded FIFO writer owning one dedicated SQLite connection.

use std::{
    any::Any,
    future::Future,
    mem,
    ops::AsyncFnOnce,
    panic::{self, AssertUnwindSafe},
    pin::Pin,
    result,
    sync::Mutex,
    thread,
};

use futures_util::FutureExt;
use tokio::{
    runtime::Builder,
    sync::{mpsc, oneshot},
    task::LocalSet,
};

use crate::{Connection, Error, Musq, Result, Transaction, TransactionBehavior};

/// Outcome returned from a serialized writer job.
enum JobOutcome<R, E> {
    /// Transaction completed with an `Ok` or `Err` result.
    Result(result::Result<R, E>),
    /// Transaction callback panicked; resumed on the caller task.
    Panicked(Box<dyn Any + Send + 'static>),
}

/// Type-erased unit of work executed sequentially on the writer connection.
trait WriterJob: Send {
    /// Execute this job on the dedicated write connection.
    fn run<'a>(self: Box<Self>, conn: &'a mut Connection) -> Pin<Box<dyn Future<Output = ()> + 'a>>;

    /// Fail this job before execution (for example, if opening the dedicated
    /// write connection failed).
    fn fail(self: Box<Self>, err: Error);
}

/// Concrete transaction job submitted to the writer thread.
struct TxJob<F, R, E> {
    /// Caller-provided async transaction callback.
    work: F,
    /// Completion channel back to the awaiting caller.
    reply: oneshot::Sender<JobOutcome<R, E>>,
}

impl<F, R, E> WriterJob for TxJob<F, R, E>
where
    F: AsyncFnOnce(&mut Transaction<&mut Connection>) -> result::Result<R, E> + Send + 'static,
    R: Send + 'static,
    E: From<Error> + Send + 'static,
{
    fn run<'a>(self: Box<Self>, conn: &'a mut Connection) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        Box::pin(async move {
            let mut tx = match conn.begin_with(TransactionBehavior::Immediate).await {
                Ok(tx) => tx,
                Err(err) => {
                    self.reply.send(JobOutcome::Result(Err(E::from(err)))).ok();
                    return;
                }
            };

            let outcome = AssertUnwindSafe((self.work)(&mut tx)).catch_unwind().await;
            match outcome {
                Ok(Ok(value)) => match tx.commit().await {
                    Ok(()) => {
                        self.reply.send(JobOutcome::Result(Ok(value))).ok();
                    }
                    Err(err) => {
                        self.reply.send(JobOutcome::Result(Err(E::from(err)))).ok();
                    }
                },
                Ok(Err(err)) => {
                    tx.rollback().await.ok();
                    self.reply.send(JobOutcome::Result(Err(err))).ok();
                }
                Err(payload) => {
                    tx.rollback().await.ok();
                    self.reply.send(JobOutcome::Panicked(payload)).ok();
                }
            }
        })
    }

    fn fail(self: Box<Self>, err: Error) {
        self.reply.send(JobOutcome::Result(Err(E::from(err)))).ok();
    }
}

/// Message sent to the dedicated writer thread.
enum WriterMessage {
    /// Execute a write transaction job in FIFO order.
    Job(Box<dyn WriterJob>),
    /// Drain prior jobs, close the write connection, and exit the thread.
    Shutdown(oneshot::Sender<Result<()>>),
}

/// Running writer thread handle.
#[derive(Debug)]
struct RunningWriter {
    /// Unbounded FIFO command channel to the writer loop.
    tx: mpsc::UnboundedSender<WriterMessage>,
}

impl RunningWriter {
    /// Spawn the dedicated single-threaded runtime and writer loop.
    fn start(write_options: Musq, thread_name: String) -> Result<Self> {
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let (tx, mut rx) = mpsc::unbounded_channel::<WriterMessage>();

        thread::Builder::new().name(thread_name).spawn(move || {
            let local = LocalSet::new();
            local.block_on(&runtime, async move {
                let mut write_conn: Option<Connection> = None;
                while let Some(message) = rx.recv().await {
                    match message {
                        WriterMessage::Job(job) => {
                            if write_conn.is_none() {
                                match Connection::connect_with(&write_options).await {
                                    Ok(conn) => write_conn = Some(conn),
                                    Err(err) => {
                                        job.fail(err);
                                        continue;
                                    }
                                }
                            }
                            if let Some(conn) = write_conn.as_mut() {
                                job.run(conn).await;
                            }
                        }
                        WriterMessage::Shutdown(reply) => {
                            let res = match write_conn.take() {
                                Some(conn) => conn.close().await,
                                None => Ok(()),
                            };
                            reply.send(res).ok();
                            return;
                        }
                    }
                }
                if let Some(conn) = write_conn.take() {
                    conn.close().await.ok();
                }
            });
        })?;

        Ok(Self { tx })
    }

    /// Gracefully drain queued writes and close the dedicated connection.
    async fn shutdown(self) -> Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self.tx.send(WriterMessage::Shutdown(reply_tx)).is_err() {
            return Ok(());
        }
        reply_rx.await.map_err(|_| Error::WorkerCrashed)?
    }
}

/// Lifecycle state of the lazy writer slot.
#[derive(Debug)]
enum WriterSlot {
    /// Writer has not started yet.
    Idle,
    /// Writer thread is running and accepting jobs.
    Running(RunningWriter),
    /// Database has been closed; reject all subsequent writes.
    Closed,
}

/// Lazy single-threaded FIFO writer shared by clones of a [`crate::Database`].
///
/// The writer thread and its dedicated SQLite connection start on the first
/// write call. Closing before any write avoids spawning the thread at all.
#[derive(Debug)]
pub(crate) struct LazyWriter {
    /// Connection options used when opening the dedicated write connection.
    write_options: Musq,
    /// Thread name for the dedicated writer runtime.
    thread_name: String,
    /// State machine protecting lazy startup and shutdown.
    slot: Mutex<WriterSlot>,
}

impl LazyWriter {
    /// Create an idle lazy writer configured with `write_options`.
    pub(crate) fn new(write_options: Musq, thread_name: String) -> Self {
        Self {
            write_options,
            thread_name,
            slot: Mutex::new(WriterSlot::Idle),
        }
    }

    /// Run `callback` inside a serialized `BEGIN IMMEDIATE` transaction on the
    /// dedicated write connection.
    pub(crate) async fn run_tx<F, R, E>(&self, callback: F) -> result::Result<R, E>
    where
        F: AsyncFnOnce(&mut Transaction<&mut Connection>) -> result::Result<R, E> + Send + 'static,
        R: Send + 'static,
        E: From<Error> + Send + 'static,
    {
        let sender = self.sender()?;
        let (reply_tx, reply_rx) = oneshot::channel();
        let job = Box::new(TxJob {
            work: callback,
            reply: reply_tx,
        });
        sender
            .send(WriterMessage::Job(job))
            .map_err(|_| Error::PoolClosed)?;

        match reply_rx.await.map_err(|_| Error::WorkerCrashed)? {
            JobOutcome::Result(res) => res,
            JobOutcome::Panicked(payload) => panic::resume_unwind(payload),
        }
    }

    /// Shut down the writer if started and mark the slot closed.
    pub(crate) async fn close(&self) -> Result<()> {
        let running = {
            let mut slot = self.slot.lock()?;
            let prev = mem::replace(&mut *slot, WriterSlot::Closed);
            match prev {
                WriterSlot::Running(running) => Some(running),
                WriterSlot::Idle | WriterSlot::Closed => None,
            }
        };
        match running {
            Some(writer) => writer.shutdown().await,
            None => Ok(()),
        }
    }

    /// Return a cloned sender for the running writer, starting it lazily on
    /// first use.
    fn sender(&self) -> Result<mpsc::UnboundedSender<WriterMessage>> {
        let mut slot = self.slot.lock()?;
        match &*slot {
            WriterSlot::Running(running) => Ok(running.tx.clone()),
            WriterSlot::Closed => Err(Error::PoolClosed),
            WriterSlot::Idle => {
                let running =
                    RunningWriter::start(self.write_options.clone(), self.thread_name.clone())?;
                let tx = running.tx.clone();
                *slot = WriterSlot::Running(running);
                Ok(tx)
            }
        }
    }
}

impl Drop for LazyWriter {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.slot.lock() {
            let prev = mem::replace(&mut *slot, WriterSlot::Closed);
            if let WriterSlot::Running(running) = prev {
                let (reply_tx, _reply_rx) = oneshot::channel();
                running.tx.send(WriterMessage::Shutdown(reply_tx)).ok();
            }
        }
    }
}
