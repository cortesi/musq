use std::{
    future::Future,
    result::Result as StdResult,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use crossbeam_queue::ArrayQueue;
use tokio::{
    sync::{Semaphore, SemaphorePermit},
    task::yield_now,
    time::timeout,
};

use super::connection::{Floating, Idle, Live};
use crate::{Error, Result};

/// get the time between the deadline and now and use that as our timeout
///
/// returns `Error::PoolTimedOut` if the deadline is in the past
fn deadline_as_timeout(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .ok_or(Error::PoolTimedOut)
}

/// Shared pool state and connection queue.
pub struct PoolInner {
    /// Idle connections waiting to be reused.
    idle_conns: ArrayQueue<Idle>,
    /// Semaphore limiting the maximum number of connections.
    semaphore: Semaphore,
    /// Current number of open connections.
    size: AtomicU32,
    /// Number of idle connections.
    num_idle: AtomicUsize,
    /// Whether the pool is closed.
    is_closed: AtomicBool,
    /// Event fired when the pool closes.
    on_closed: event_listener::Event,
    /// Pool configuration options.
    pub(super) options: crate::Musq,
}

impl PoolInner {
    /// Create a new shared pool state.
    pub(super) fn new_arc(options: crate::Musq) -> Arc<Self> {
        Arc::new(Self {
            idle_conns: ArrayQueue::new(options.pool_max_connections as usize),
            semaphore: Semaphore::new(options.pool_max_connections as usize),
            size: AtomicU32::new(0),
            num_idle: AtomicUsize::new(0),
            is_closed: AtomicBool::new(false),
            on_closed: event_listener::Event::new(),
            options,
        })
    }

    /// Return the current number of open connections.
    pub(super) fn size(&self) -> u32 {
        self.size.load(Ordering::Acquire)
    }

    /// Return the number of idle connections.
    pub(super) fn num_idle(&self) -> usize {
        // We don't use `self.idle_conns.len()` as it waits for the internal
        // head and tail pointers to stop changing for a moment before calculating the length,
        // which may take a long time at high levels of churn.
        //
        // By maintaining our own atomic count, we avoid that issue entirely.
        self.num_idle.load(Ordering::Acquire)
    }

    /// Returns `true` if the pool is closed.
    pub(super) fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Acquire)
    }

    /// Mark the pool as closed and notify listeners.
    fn mark_closed(&self) {
        self.is_closed.store(true, Ordering::Release);
        self.on_closed.notify(usize::MAX);
    }

    /// Close the pool and wait for connections to drain.
    pub(super) async fn close(self: &Arc<Self>) {
        self.mark_closed();

        for permits in 1..=self.options.pool_max_connections {
            // Close any currently idle connections in the pool.
            while let Some(idle) = self.idle_conns.pop() {
                let _ = idle.live.float((*self).clone()).close().await;
            }

            if self.size() == 0 {
                break;
            }

            // Wait for all permits to be released.
            if let Err(err) = self.semaphore.acquire_many(permits).await {
                tracing::warn!("semaphore closed while waiting for permits: {err:?}");
                break;
            }
        }
    }

    /// Future that resolves when the pool closes.
    pub(crate) fn close_event(&self) -> impl Future<Output = ()> + '_ {
        let listener = (!self.is_closed()).then(|| self.on_closed.listen());

        async move {
            if let Some(listener) = listener {
                listener.await;
            }
        }
    }

    /// Attempt to acquire a permit from `self.semaphore`.
    ///
    /// If the pool is closed while waiting, an error is returned.
    async fn acquire_permit<'a>(self: &'a Arc<Self>) -> Result<SemaphorePermit<'a>> {
        tokio::select! {
            permit = self.semaphore.acquire_many(1) => {
                match permit {
                    Ok(permit) => Ok(permit),
                    Err(_) => Err(Error::PoolClosed),
                }
            }
            _ = self.close_event() => Err(Error::PoolClosed),
        }
    }

    /// Attempt to acquire an idle connection without waiting.
    pub(super) fn try_acquire(self: &Arc<Self>) -> Option<Floating<Idle>> {
        if self.is_closed() {
            return None;
        }

        let permit = self.semaphore.try_acquire_many(1).ok()?;

        self.pop_idle(permit).ok()
    }

    /// Pop an idle connection or return the permit.
    fn pop_idle<'a>(
        self: &'a Arc<Self>,
        permit: SemaphorePermit<'a>,
    ) -> StdResult<Floating<Idle>, SemaphorePermit<'a>> {
        if let Some(idle) = self.idle_conns.pop() {
            self.num_idle.fetch_sub(1, Ordering::AcqRel);
            Ok(Floating::from_idle(idle, (*self).clone(), permit))
        } else {
            Err(permit)
        }
    }

    /// Release a live connection back to the idle queue.
    pub(super) fn release(&self, floating: Floating<Live>) {
        let Floating { inner: idle, guard } = floating.into_idle();

        if self.idle_conns.push(idle).is_err() {
            panic!("BUG: connection queue overflow in release()");
        }

        // NOTE: we need to make sure we drop the permit *after* we push to the idle queue
        // don't decrease the size
        guard.release_permit();

        self.num_idle.fetch_add(1, Ordering::AcqRel);
    }

    /// Try to atomically increment the pool size for a new connection.
    ///
    /// Returns `Err` if the pool is at max capacity already or is closed.
    fn try_increment_size<'a>(
        self: &'a Arc<Self>,
        permit: SemaphorePermit<'a>,
    ) -> StdResult<DecrementSizeGuard, SemaphorePermit<'a>> {
        match self
            .size
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |size| {
                if self.is_closed() {
                    return None;
                }

                size.checked_add(1)
                    .filter(|size| size <= &self.options.pool_max_connections)
            }) {
            // we successfully incremented the size
            Ok(_) => Ok(DecrementSizeGuard::from_permit((*self).clone(), permit)),
            // the pool is at max capacity or is closed
            Err(_) => Err(permit),
        }
    }

    /// Acquire a live connection, waiting until one is available.
    pub(super) async fn acquire(self: &Arc<Self>) -> Result<Floating<Live>> {
        if self.is_closed() {
            return Err(Error::PoolClosed);
        }

        let deadline = Instant::now() + self.options.pool_acquire_timeout;

        timeout(
            self.options.pool_acquire_timeout,
            async {
                loop {
                    // Handles the close-event internally
                    let permit = self.acquire_permit().await?;

                    // First attempt to pop a connection from the idle queue.
                    let guard = match self.pop_idle(permit) {

                        // Then, check that we can use it...
                        Ok(conn) => return Ok(conn.into_live()),
                        Err(permit) => if let Ok(guard) = self.try_increment_size(permit) {
                            // we can open a new connection
                            guard
                        } else {
                            // This can happen if the pool is already at its connection limit
                            // or if it was closed between `acquire_permit()` and
                            // `try_increment_size()`.
                            tracing::debug!("woke but was unable to acquire idle connection or open new one; retrying");
                            // If so, we're likely in the current-thread runtime if it's Tokio
                            // and so we should yield to let any spawned release_to_pool() tasks
                            // execute.
                            yield_now().await;
                            continue;
                        }
                    };

                    // Attempt to connect...
                    return self.connect(deadline, guard).await;
                }
            }
        )
            .await
            .map_err(|_| Error::PoolTimedOut)?
    }

    /// Establish a new live connection with a timeout.
    async fn connect(
        self: &Arc<Self>,
        deadline: Instant,
        guard: DecrementSizeGuard,
    ) -> Result<Floating<Live>> {
        if self.is_closed() {
            return Err(Error::PoolClosed);
        }
        let timeout_duration = deadline_as_timeout(deadline)?;

        // result here is `Result<Result<C>, TimeoutError>`
        // if this block does not return, sleep for the backoff timeout and try again
        match timeout(timeout_duration, self.options.connect()).await {
            Ok(Ok(raw)) => Ok(Floating::new_live(raw, guard)),
            Ok(Err(e)) => Err(e),
            // timed out
            Err(_) => Err(Error::PoolTimedOut),
        }
    }
}

impl Drop for PoolInner {
    fn drop(&mut self) {
        self.mark_closed();
    }
}

/// RAII guard returned by `Pool::try_increment_size()` and others.
///
/// Will decrement the pool size if dropped, to avoid semantically "leaking" connections
/// (where the pool thinks it has more connections than it does).
pub(in crate::pool) struct DecrementSizeGuard {
    /// Owning pool reference.
    pub(crate) pool: Arc<PoolInner>,
    /// Whether the guard has been cancelled.
    cancelled: bool,
}

impl DecrementSizeGuard {
    /// Create a new guard that will release a semaphore permit on-drop.
    pub fn new_permit(pool: Arc<PoolInner>) -> Self {
        Self {
            pool,
            cancelled: false,
        }
    }

    /// Create a guard from an acquired semaphore permit.
    pub fn from_permit(pool: Arc<PoolInner>, permit: SemaphorePermit<'_>) -> Self {
        // here we effectively take ownership of the permit
        permit.forget();
        Self::new_permit(pool)
    }

    /// Release the semaphore permit without decreasing the pool size.
    fn release_permit(self) {
        self.pool.semaphore.add_permits(1);
        self.cancel();
    }

    /// Cancel decrementing the pool size on drop.
    pub fn cancel(mut self) {
        self.cancelled = true;
    }
}

impl Drop for DecrementSizeGuard {
    fn drop(&mut self) {
        if !self.cancelled {
            self.pool.size.fetch_sub(1, Ordering::AcqRel);

            // and here we release the permit we got on construction
            self.pool.semaphore.add_permits(1);
        }
    }
}
