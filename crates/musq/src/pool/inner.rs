use std::{
    future::Future,
    result,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    },
};

use crossbeam_queue::ArrayQueue;
use tokio::{
    sync::{Semaphore, SemaphorePermit, watch},
    task::yield_now,
    time::timeout,
};

use super::connection::Floating;
use crate::{Connection, Error, Result};

/// Shared pool state and connection queue.
pub struct PoolInner {
    /// Idle connections waiting to be reused.
    idle_conns: ArrayQueue<Connection>,
    /// Semaphore limiting the maximum number of connections.
    semaphore: Semaphore,
    /// Current number of open connections.
    size: AtomicU32,
    /// Number of idle connections.
    num_idle: AtomicUsize,
    /// Whether the pool is closed.
    is_closed: AtomicBool,
    /// Broadcast channel signalling pool closure.
    on_closed: watch::Sender<bool>,
    /// Pool configuration options.
    pub(super) options: crate::Musq,
}

impl PoolInner {
    /// Create a new shared pool state.
    pub(super) fn new_arc(options: crate::Musq) -> Arc<Self> {
        let (on_closed, _) = watch::channel(false);
        Arc::new(Self {
            idle_conns: ArrayQueue::new(options.pool_max_connections as usize),
            semaphore: Semaphore::new(options.pool_max_connections as usize),
            size: AtomicU32::new(0),
            num_idle: AtomicUsize::new(0),
            is_closed: AtomicBool::new(false),
            on_closed,
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
        // head and tail pointers to stop changing for a moment before
        // calculating the length, which may take a long time at high
        // levels of churn.
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
        self.on_closed.send_replace(true);
    }

    /// Close the pool and wait for connections to drain.
    pub(super) async fn close(self: &Arc<Self>) {
        self.mark_closed();

        loop {
            self.close_idle_connections().await;

            let size = self.size();
            if size == 0 {
                return;
            }

            // After all idle connections have been drained, the semaphore
            // already contains one available permit for every
            // closed/absent connection. Wait for one more permit
            // than that baseline, which means a checked-out
            // connection has either returned or closed.
            let permits = self
                .options
                .pool_max_connections
                .saturating_sub(size)
                .saturating_add(1);
            if let Err(err) = self.semaphore.acquire_many(permits).await {
                tracing::warn!("semaphore closed while waiting for permits: {err:?}");
                return;
            }
        }
    }

    /// Close all idle connections currently sitting in the idle queue.
    async fn close_idle_connections(&self) {
        while let Some(conn) = self.idle_conns.pop() {
            self.num_idle.fetch_sub(1, Ordering::AcqRel);
            let _close_result = conn.close().await;
            self.size.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Future that resolves when the pool closes.
    pub(crate) fn close_event(&self) -> impl Future<Output = ()> + '_ {
        let mut rx = self.on_closed.subscribe();
        async move {
            rx.wait_for(|&closed| closed).await.ok();
        }
    }

    /// Attempt to acquire a permit from `self.semaphore`.
    ///
    /// If the pool is closed while waiting, an error is returned.
    async fn acquire_permit<'a>(self: &'a Arc<Self>) -> Result<SemaphorePermit<'a>> {
        tokio::select! {
            biased;
            _ = self.close_event() => Err(Error::PoolClosed),
            permit = self.semaphore.acquire() => {
                permit.map_err(|_| Error::PoolClosed)
            }
        }
    }

    /// Attempt to acquire an idle connection without waiting.
    pub(super) fn try_acquire(self: &Arc<Self>) -> Option<Floating> {
        if self.is_closed() {
            return None;
        }

        let permit = self.semaphore.try_acquire().ok()?;

        self.pop_idle(permit).ok()
    }

    /// Pop an idle connection or return the permit.
    fn pop_idle<'a>(
        self: &'a Arc<Self>,
        permit: SemaphorePermit<'a>,
    ) -> result::Result<Floating, SemaphorePermit<'a>> {
        if let Some(conn) = self.idle_conns.pop() {
            self.num_idle.fetch_sub(1, Ordering::AcqRel);
            Ok(Floating::from_idle(conn, (*self).clone(), permit))
        } else {
            Err(permit)
        }
    }

    /// Release a floating connection back to the idle queue.
    pub(super) fn release(&self, floating: Floating) {
        let Floating { conn, guard } = floating;
        guard.cancel();
        self.release_conn(conn);
    }

    /// Return a connection whose permit is already checked out to the idle
    /// queue.
    pub(super) fn release_conn(&self, conn: Connection) {
        // Increment before pushing so a concurrent pop cannot underflow
        // `num_idle` below zero.
        self.num_idle.fetch_add(1, Ordering::AcqRel);
        if self.idle_conns.push(conn).is_err() {
            self.num_idle.fetch_sub(1, Ordering::AcqRel);
            panic!("BUG: connection queue overflow in release()");
        }

        // Release the permit after pushing to the idle queue so waiters can
        // pop the connection immediately.
        self.semaphore.add_permits(1);
    }

    /// Try to atomically increment the pool size for a new connection.
    ///
    /// Returns `Err` if the pool is at max capacity already or is closed.
    fn try_increment_size<'a>(
        self: &'a Arc<Self>,
        permit: SemaphorePermit<'a>,
    ) -> result::Result<DecrementSizeGuard, SemaphorePermit<'a>> {
        match self
            .size
            .try_update(Ordering::AcqRel, Ordering::Acquire, |size| {
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
    pub(super) async fn acquire(self: &Arc<Self>) -> Result<Floating> {
        if self.is_closed() {
            return Err(Error::PoolClosed);
        }

        timeout(
            self.options.pool_acquire_timeout,
            async {
                loop {
                    // Handles the close-event internally
                    let permit = self.acquire_permit().await?;

                    // First attempt to pop a connection from the idle queue.
                    let guard = match self.pop_idle(permit) {
                        Ok(conn) => return Ok(conn),
                        Err(permit) => if let Ok(guard) = self.try_increment_size(permit) {
                            // we can open a new connection
                            guard
                        } else {
                            // This can happen if the pool is already at its connection limit
                            // or if it was closed between `acquire_permit()` and
                            // `try_increment_size()`.
                            tracing::debug!("woke but was unable to acquire idle connection or open new one; retrying");
                            yield_now().await;
                            continue;
                        }
                    };

                    return self.connect(guard).await;
                }
            }
        )
            .await
            .map_err(|_| Error::PoolTimedOut)?
    }

    /// Establish a new live connection.
    async fn connect(self: &Arc<Self>, guard: DecrementSizeGuard) -> Result<Floating> {
        if self.is_closed() {
            return Err(Error::PoolClosed);
        }

        let raw = self.options.connect().await?;
        Ok(Floating::new(raw, guard))
    }
}

impl Drop for PoolInner {
    fn drop(&mut self) {
        self.mark_closed();
    }
}

/// RAII guard returned by `Pool::try_increment_size()` and others.
///
/// Will decrement the pool size if dropped, to avoid semantically "leaking"
/// connections (where the pool thinks it has more connections than it does).
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
