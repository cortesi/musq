use crate::{
    error::{Error, Result},
    pool::{inner::PoolInner, Pool},
    ConnectOptions,
};

use std::{
    fmt::Debug,
    path::Path,
    time::{Duration, Instant},
};

/// Configuration options for [`Pool`][super::Pool].
#[derive(Debug, Clone)]
pub struct PoolOptions {
    pub(crate) max_connections: u32,
    pub(crate) acquire_timeout: Duration,
    pub(crate) min_connections: u32,
    pub(crate) max_lifetime: Option<Duration>,
    pub(crate) idle_timeout: Option<Duration>,
    pub(crate) connect_options: ConnectOptions,
}

impl PoolOptions {
    /// Returns a default "sane" configuration, suitable for testing or light-duty applications.
    ///
    /// Production applications will likely want to at least modify
    /// [`max_connections`][Self::max_connections].
    ///
    /// See the source of this method for the current default values.
    pub(crate) fn new(connect_options: ConnectOptions) -> Self {
        Self {
            // A production application will want to set a higher limit than this.
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(10 * 60)),
            max_lifetime: Some(Duration::from_secs(30 * 60)),
            connect_options,
        }
    }

    /// Set the maximum number of connections that this pool should maintain.
    ///
    /// Be mindful of the connection limits for your database as well as other applications
    /// which may want to connect to the same database (or even multiple instances of the same
    /// application in high-availability deployments).
    pub fn max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    /// Get the maximum number of connections that this pool should maintain
    pub fn get_max_connections(&self) -> u32 {
        self.max_connections
    }

    /// Set the minimum number of connections to maintain at all times.
    ///
    /// When the pool is built, this many connections will be automatically spun up.
    ///
    /// If any connection is reaped by [`max_lifetime`] or [`idle_timeout`], or explicitly closed,
    /// and it brings the connection count below this amount, a new connection will be opened to
    /// replace it.
    ///
    /// This is only done on a best-effort basis, however. The routine that maintains this value
    /// has a deadline so it doesn't wait forever if the database is being slow or returning errors.
    ///
    /// This value is clamped internally to not exceed [`max_connections`].
    ///
    /// We've chosen not to assert `min_connections <= max_connections` anywhere
    /// because it shouldn't break anything internally if the condition doesn't hold,
    /// and if the application allows either value to be dynamically set
    /// then it should be checking this condition itself and returning
    /// a nicer error than a panic anyway.
    ///
    /// [`max_lifetime`]: Self::max_lifetime
    /// [`idle_timeout`]: Self::idle_timeout
    /// [`max_connections`]: Self::max_connections
    pub fn min_connections(mut self, min: u32) -> Self {
        self.min_connections = min;
        self
    }

    /// Get the minimum number of connections to maintain at all times.
    pub fn get_min_connections(&self) -> u32 {
        self.min_connections
    }

    /// Set the maximum amount of time to spend waiting for a connection in [`Pool::acquire()`].
    ///
    /// Caps the total amount of time `Pool::acquire()` can spend waiting across multiple phases:
    ///
    /// * First, it may need to wait for a permit from the semaphore, which grants it the privilege
    ///   of opening a connection or popping one from the idle queue.
    /// * If an existing idle connection is acquired, by default it will be checked for liveness
    ///   and integrity before being returned, which may require executing a command on the
    ///   connection. This can be disabled with [`test_before_acquire(false)`][Self::test_before_acquire].
    ///     * If [`before_acquire`][Self::before_acquire] is set, that will also be executed.
    /// * If a new connection needs to be opened, that will obviously require I/O, handshaking,
    ///   and initialization commands.
    ///     * If [`after_connect`][Self::after_connect] is set, that will also be executed.
    pub fn acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    /// Get the maximum amount of time to spend waiting for a connection in [`Pool::acquire()`].
    pub fn get_acquire_timeout(&self) -> Duration {
        self.acquire_timeout
    }

    /// Set the maximum lifetime of individual connections.
    ///
    /// Any connection with a lifetime greater than this will be closed.
    ///
    /// When set to `None`, all connections live until either reaped by [`idle_timeout`]
    /// or explicitly disconnected.
    ///
    /// Infinite connections are not recommended due to the unfortunate reality of memory/resource
    /// leaks on the database-side. It is better to retire connections periodically
    /// (even if only once daily) to allow the database the opportunity to clean up data structures
    /// (parse trees, query metadata caches, thread-local storage, etc.) that are associated with a
    /// session.
    ///
    /// [`idle_timeout`]: Self::idle_timeout
    pub fn max_lifetime(mut self, lifetime: impl Into<Option<Duration>>) -> Self {
        self.max_lifetime = lifetime.into();
        self
    }

    /// Get the maximum lifetime of individual connections.
    pub fn get_max_lifetime(&self) -> Option<Duration> {
        self.max_lifetime
    }

    /// Set a maximum idle duration for individual connections.
    ///
    /// Any connection that remains in the idle queue longer than this will be closed.
    ///
    /// For usage-based database server billing, this can be a cost saver.
    pub fn idle_timeout(mut self, timeout: impl Into<Option<Duration>>) -> Self {
        self.idle_timeout = timeout.into();
        self
    }

    /// Get the maximum idle duration for individual connections.
    pub fn get_idle_timeout(&self) -> Option<Duration> {
        self.idle_timeout
    }

    /// Create a new pool from this `PoolOptions` and immediately open at least one connection.
    ///
    /// This ensures the configuration is correct.
    ///
    /// The total number of connections opened is <code>min(1, [min_connections][Self::min_connections])</code>.
    pub async fn connect(self) -> Result<Pool, Error> {
        // Don't take longer than `acquire_timeout` starting from when this is called.
        let deadline = Instant::now() + self.acquire_timeout;
        let inner = PoolInner::new_arc(self);
        if inner.options.min_connections > 0 {
            // If the idle reaper is spawned then this will race with the call from that task
            // and may not report any connection errors.
            inner.try_min_connections(deadline).await?;
        }
        // If `min_connections` is nonzero then we'll likely just pull a connection
        // from the idle queue here, but it should at least get tested first.
        let conn = inner.acquire().await?;
        inner.release(conn);
        Ok(Pool(inner))
    }

    /// Open a file
    pub async fn open(mut self, filename: impl AsRef<Path>) -> Result<Pool> {
        self.connect_options = self.connect_options.filename(filename);
        self.connect().await
    }

    /// Open an in-memory database
    pub async fn open_in_memory(mut self) -> Result<Pool> {
        self.connect_options = self.connect_options.configure_in_memory();
        self.connect().await
    }
}
