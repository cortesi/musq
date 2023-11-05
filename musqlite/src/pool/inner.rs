use std::{
    cmp,
    future::Future,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crossbeam_queue::ArrayQueue;
use futures_util::FutureExt;

use super::connection::{Floating, Idle, Live};
use crate::{
    error::Error,
    pool::{deadline_as_timeout, CloseEvent, PoolOptions},
};

pub(crate) struct PoolInner {
    pub(super) idle_conns: ArrayQueue<Idle>,
    pub(super) semaphore: tokio::sync::Semaphore,
    pub(super) size: AtomicU32,
    pub(super) num_idle: AtomicUsize,
    is_closed: AtomicBool,
    pub(super) on_closed: event_listener::Event,
    pub(super) options: PoolOptions,
}

impl PoolInner {
    pub(super) fn new_arc(options: PoolOptions) -> Arc<Self> {
        let capacity = options.max_connections as usize;
        let semaphore_capacity = capacity;

        let pool = Self {
            idle_conns: ArrayQueue::new(capacity),
            semaphore: tokio::sync::Semaphore::new(semaphore_capacity),
            size: AtomicU32::new(0),
            num_idle: AtomicUsize::new(0),
            is_closed: AtomicBool::new(false),
            on_closed: event_listener::Event::new(),
            options,
        };

        let pool = Arc::new(pool);

        spawn_maintenance_tasks(&pool);

        pool
    }

    pub(super) fn size(&self) -> u32 {
        self.size.load(Ordering::Acquire)
    }

    pub(super) fn num_idle(&self) -> usize {
        // We don't use `self.idle_conns.len()` as it waits for the internal
        // head and tail pointers to stop changing for a moment before calculating the length,
        // which may take a long time at high levels of churn.
        //
        // By maintaining our own atomic count, we avoid that issue entirely.
        self.num_idle.load(Ordering::Acquire)
    }

    pub(super) fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Acquire)
    }

    fn mark_closed(&self) {
        self.is_closed.store(true, Ordering::Release);
        self.on_closed.notify(usize::MAX);
    }

    pub(super) fn close<'a>(self: &'a Arc<Self>) -> impl Future<Output = ()> + 'a {
        self.mark_closed();

        async move {
            for permits in 1..=self.options.max_connections {
                // Close any currently idle connections in the pool.
                while let Some(idle) = self.idle_conns.pop() {
                    let _ = idle.live.float((*self).clone()).close().await;
                }

                if self.size() == 0 {
                    break;
                }

                // Wait for all permits to be released.
                let _permits = self.semaphore.acquire_many(permits).await.unwrap();
            }
        }
    }

    pub(crate) fn close_event(&self) -> CloseEvent {
        CloseEvent {
            listener: (!self.is_closed()).then(|| self.on_closed.listen()),
        }
    }

    /// Attempt to pull a permit from `self.semaphore` or steal one from the parent.
    ///
    /// If we steal a permit from the parent but *don't* open a connection,
    /// it should be returned to the parent.
    async fn acquire_permit<'a>(
        self: &'a Arc<Self>,
    ) -> Result<tokio::sync::SemaphorePermit<'a>, Error> {
        let acquire_self = self.semaphore.acquire_many(1).fuse();
        let mut close_event = self.close_event();
        close_event.do_until(acquire_self).await.map(|e| e.unwrap())
    }

    #[inline]
    pub(super) fn try_acquire(self: &Arc<Self>) -> Option<Floating<Idle>> {
        if self.is_closed() {
            return None;
        }

        let permit = self.semaphore.try_acquire_many(1).ok()?;

        self.pop_idle(permit).ok()
    }

    fn pop_idle<'a>(
        self: &'a Arc<Self>,
        permit: tokio::sync::SemaphorePermit<'a>,
    ) -> Result<Floating<Idle>, tokio::sync::SemaphorePermit<'a>> {
        if let Some(idle) = self.idle_conns.pop() {
            self.num_idle.fetch_sub(1, Ordering::AcqRel);
            Ok(Floating::from_idle(idle, (*self).clone(), permit))
        } else {
            Err(permit)
        }
    }

    pub(super) fn release(&self, floating: Floating<Live>) {
        // `options.after_release` is invoked by `PoolConnection::release_to_pool()`.

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
    pub(super) fn try_increment_size<'a>(
        self: &'a Arc<Self>,
        permit: tokio::sync::SemaphorePermit<'a>,
    ) -> Result<DecrementSizeGuard, tokio::sync::SemaphorePermit<'a>> {
        match self
            .size
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |size| {
                if self.is_closed() {
                    return None;
                }

                size.checked_add(1)
                    .filter(|size| size <= &self.options.max_connections)
            }) {
            // we successfully incremented the size
            Ok(_) => Ok(DecrementSizeGuard::from_permit((*self).clone(), permit)),
            // the pool is at max capacity or is closed
            Err(_) => Err(permit),
        }
    }

    pub(super) async fn acquire(self: &Arc<Self>) -> Result<Floating<Live>, Error> {
        if self.is_closed() {
            return Err(Error::PoolClosed);
        }

        let deadline = Instant::now() + self.options.acquire_timeout;

        tokio::time::timeout(
            self.options.acquire_timeout,
            async {
                loop {
                    // Handles the close-event internally
                    let permit = self.acquire_permit().await?;


                    // First attempt to pop a connection from the idle queue.
                    let guard = match self.pop_idle(permit) {

                        // Then, check that we can use it...
                        Ok(conn) => match check_idle_conn(conn, &self.options).await {

                            // All good!
                            Ok(live) => return Ok(live),

                            // if the connection isn't usable for one reason or another,
                            // we get the `DecrementSizeGuard` back to open a new one
                            Err(guard) => guard,
                        },
                        Err(permit) => if let Ok(guard) = self.try_increment_size(permit) {
                            // we can open a new connection
                            guard
                        } else {
                            // This can happen for a child pool that's at its connection limit,
                            // or if the pool was closed between `acquire_permit()` and
                            // `try_increment_size()`.
                            tracing::debug!("woke but was unable to acquire idle connection or open new one; retrying");
                            // If so, we're likely in the current-thread runtime if it's Tokio
                            // and so we should yield to let any spawned release_to_pool() tasks
                            // execute.
                            tokio::task::yield_now().await;
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

    pub(super) async fn connect(
        self: &Arc<Self>,
        deadline: Instant,
        guard: DecrementSizeGuard,
    ) -> Result<Floating<Live>, Error> {
        if self.is_closed() {
            return Err(Error::PoolClosed);
        }

        let mut backoff = Duration::from_millis(10);
        let max_backoff = deadline_as_timeout(deadline)? / 5;

        loop {
            let timeout = deadline_as_timeout(deadline)?;

            // clone the connect options arc so it can be used without holding the RwLockReadGuard
            // across an async await point
            let connect_options = self.options.connect_options.clone();

            // result here is `Result<Result<C, Error>, TimeoutError>`
            // if this block does not return, sleep for the backoff timeout and try again
            match tokio::time::timeout(timeout, connect_options.connect()).await {
                // successfully established connection
                Ok(Ok(raw)) => {
                    return Ok(Floating::new_live(raw, guard));
                }

                // an IO error while connecting is assumed to be the system starting up
                Ok(Err(Error::Io(e))) if e.kind() == std::io::ErrorKind::ConnectionRefused => (),

                // Any other error while connection should immediately
                // terminate and bubble the error up
                Ok(Err(e)) => return Err(e),

                // timed out
                Err(_) => return Err(Error::PoolTimedOut),
            }

            // If the connection is refused, wait in exponentially
            // increasing steps for the server to come up,
            // capped by a factor of the remaining time until the deadline
            tokio::time::sleep(backoff).await;
            backoff = cmp::min(backoff * 2, max_backoff);
        }
    }

    /// Try to maintain `min_connections`, returning any errors (including `PoolTimedOut`).
    pub async fn try_min_connections(self: &Arc<Self>, deadline: Instant) -> Result<(), Error> {
        while self.size() < self.options.min_connections {
            // Don't wait for a semaphore permit.
            //
            // If no extra permits are available then we shouldn't be trying to spin up
            // connections anyway.
            let Ok(permit) = self.semaphore.try_acquire_many(1) else {
                return Ok(());
            };

            // We must always obey `max_connections`.
            let Some(guard) = self.try_increment_size(permit).ok() else {
                return Ok(());
            };

            // We skip `after_release` since the connection was never provided to user code
            // besides `after_connect`, if they set it.
            self.release(self.connect(deadline, guard).await?);
        }

        Ok(())
    }

    /// Attempt to maintain `min_connections`, logging if unable.
    pub async fn min_connections_maintenance(self: &Arc<Self>, deadline: Option<Instant>) {
        let deadline = deadline.unwrap_or_else(|| {
            // Arbitrary default deadline if the caller doesn't care.
            Instant::now() + Duration::from_secs(300)
        });

        match self.try_min_connections(deadline).await {
            Ok(()) => (),
            Err(Error::PoolClosed) => (),
            Err(Error::PoolTimedOut) => {
                tracing::debug!("unable to complete `min_connections` maintenance before deadline")
            }
            Err(error) => tracing::debug!(%error, "error while maintaining min_connections"),
        }
    }
}

impl Drop for PoolInner {
    fn drop(&mut self) {
        self.mark_closed();
    }
}

/// Returns `true` if the connection has exceeded `options.max_lifetime` if set, `false` otherwise.
fn is_beyond_max_lifetime(live: &Live, options: &PoolOptions) -> bool {
    options
        .max_lifetime
        .map_or(false, |max| live.created_at.elapsed() > max)
}

/// Returns `true` if the connection has exceeded `options.idle_timeout` if set, `false` otherwise.
fn is_beyond_idle_timeout(idle: &Idle, options: &PoolOptions) -> bool {
    options
        .idle_timeout
        .map_or(false, |timeout| idle.idle_since.elapsed() > timeout)
}

async fn check_idle_conn(
    conn: Floating<Idle>,
    options: &PoolOptions,
) -> Result<Floating<Live>, DecrementSizeGuard> {
    // If the connection we pulled has expired, close the connection and
    // immediately create a new connection
    if is_beyond_max_lifetime(&conn, options) {
        return Err(conn.close().await);
    }

    // No need to re-connect; connection is alive or we don't care
    Ok(conn.into_live())
}

fn spawn_maintenance_tasks(pool: &Arc<PoolInner>) {
    // NOTE: use `pool_weak` for the maintenance tasks so
    // they don't keep `PoolInner` from being dropped.
    let pool_weak = Arc::downgrade(pool);

    let period = match (pool.options.max_lifetime, pool.options.idle_timeout) {
        (Some(it), None) | (None, Some(it)) => it,

        (Some(a), Some(b)) => cmp::min(a, b),

        (None, None) => {
            if pool.options.min_connections > 0 {
                tokio::task::spawn(async move {
                    if let Some(pool) = pool_weak.upgrade() {
                        pool.min_connections_maintenance(None).await;
                    }
                });
            }

            return;
        }
    };

    // Immediately cancel this task if the pool is closed.
    let mut close_event = pool.close_event();

    tokio::task::spawn(async move {
        let _ = close_event
            .do_until(async {
                let mut slept = true;

                // If the last handle to the pool was dropped while we were sleeping
                while let Some(pool) = pool_weak.upgrade() {
                    if pool.is_closed() {
                        return;
                    }

                    // Don't run the reaper right away.
                    if slept && !pool.idle_conns.is_empty() {
                        do_reap(&pool).await;
                    }

                    let next_run = Instant::now() + period;

                    pool.min_connections_maintenance(Some(next_run)).await;

                    // Don't hold a reference to the pool while sleeping.
                    drop(pool);

                    if let Some(duration) = next_run.checked_duration_since(Instant::now()) {
                        // `async-std` doesn't have a `sleep_until()`
                        tokio::time::sleep(duration).await;
                    } else {
                        // `next_run` is in the past, just yield.
                        tokio::task::yield_now().await;
                    }

                    slept = true;
                }
            })
            .await;
    });
}

async fn do_reap(pool: &Arc<PoolInner>) {
    // reap at most the current size minus the minimum idle
    let max_reaped = pool.size().saturating_sub(pool.options.min_connections);

    // collect connections to reap
    let (reap, keep) = (0..max_reaped)
        // only connections waiting in the queue
        .filter_map(|_| pool.try_acquire())
        .partition::<Vec<_>, _>(|conn| {
            is_beyond_idle_timeout(conn, &pool.options)
                || is_beyond_max_lifetime(conn, &pool.options)
        });

    for conn in keep {
        // return valid connections to the pool first
        pool.release(conn.into_live());
    }

    for conn in reap {
        let _ = conn.close().await;
    }
}

/// RAII guard returned by `Pool::try_increment_size()` and others.
///
/// Will decrement the pool size if dropped, to avoid semantically "leaking" connections
/// (where the pool thinks it has more connections than it does).
pub(in crate::pool) struct DecrementSizeGuard {
    pub(crate) pool: Arc<PoolInner>,
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

    pub fn from_permit(pool: Arc<PoolInner>, permit: tokio::sync::SemaphorePermit<'_>) -> Self {
        // here we effectively take ownership of the permit
        permit.forget();
        Self::new_permit(pool)
    }

    /// Release the semaphore permit without decreasing the pool size.
    ///
    /// If the permit was stolen from the pool's parent, it will be returned to the child's semaphore.
    fn release_permit(self) {
        self.pool.semaphore.add_permits(1);
        self.cancel();
    }

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
