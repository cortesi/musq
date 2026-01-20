use std::{
    fmt::{self, Debug, Formatter},
    future::Future,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use tokio::{runtime::Handle, sync::SemaphorePermit};

use super::inner::{DecrementSizeGuard, PoolInner};
use crate::{Connection, Result};

/// A single database connection acquired from a [`Pool`].
///
/// A `PoolConnection` represents exclusive access to a single physical database connection.
/// Its primary purpose is to handle sequences of operations that must run on the same
/// connection but are not part of a formal transaction.
///
/// For most database interactions, it is more convenient to execute queries directly on a
/// [`&Pool`] or to use a [`Transaction`].
///
/// A `PoolConnection` is automatically returned to the pool when it is dropped.
///
/// ### Use Case: Temporary Tables
///
/// The most common reason to manually acquire a `PoolConnection` is to work with temporary
/// tables, which are only visible to the connection that created them.
///
/// ```rust,ignore
/// use musq::{sql, Pool};
///
/// async fn work_with_temp_table(pool: &Pool) -> musq::Result<()> {
///     // Acquire a single connection to ensure all commands see the temp table.
///     let mut conn = pool.acquire().await?;
///
///     sql!("CREATE TEMP TABLE temp_users (id INTEGER);")
///         .execute(&mut conn)
///         .await?;
///
///     sql!("INSERT INTO temp_users (id) VALUES (1), (2);")
///         .execute(&mut conn)
///         .await?;
///
///     // If these queries were run directly on the pool, they might be assigned
///     // different connections and fail to see `temp_users`.
///
///     Ok(())
/// } // conn is dropped here and returned to the pool.
/// ```
pub struct PoolConnection {
    /// Live connection, if still attached.
    live: Option<Live>,
    /// Owning pool reference.
    pool: Arc<PoolInner>,
}

/// Live connection wrapper.
pub(super) struct Live {
    /// Underlying connection handle.
    raw: Connection,
}

/// Idle connection wrapper.
pub(super) struct Idle {
    /// Live connection state.
    pub(super) live: Live,
}

/// RAII wrapper for connections being handled by functions that may drop them.
pub(super) struct Floating<C> {
    /// Wrapped connection state.
    pub(super) inner: C,
    /// Guard that decrements pool size on drop.
    pub(super) guard: DecrementSizeGuard,
}

/// Error message for missing pooled connection state.
const EXPECT_MSG: &str = "BUG: inner connection already taken!";

impl Debug for PoolConnection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // TODO: Show the type name of the connection ?
        f.debug_struct("PoolConnection").finish()
    }
}

impl Deref for PoolConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.live.as_ref().expect(EXPECT_MSG).raw
    }
}

impl DerefMut for PoolConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.live.as_mut().expect(EXPECT_MSG).raw
    }
}

impl PoolConnection {
    /// Close this connection, allowing the pool to open a replacement.
    ///
    /// The connection permit is retained for the duration so the pool will not
    /// exceed `max_connections`.
    ///
    /// The returned future **must** be awaited to ensure the connection is
    /// fully closed.
    #[must_use = "futures returned by `PoolConnection::close` must be awaited"]
    pub async fn close(mut self) -> Result<()> {
        let floating = self.take_live().float(self.pool.clone());
        floating.inner.raw.close().await
    }

    /// Take ownership of the live connection, if present.
    fn take_live(&mut self) -> Live {
        self.live.take().expect(EXPECT_MSG)
    }

    /// Test the connection to make sure it is still live before returning it to the pool.
    ///
    /// This effectively runs the drop handler eagerly instead of spawning a task to do it.
    #[doc(hidden)]
    pub(crate) fn return_to_pool(&mut self) -> impl Future<Output = ()> + Send + 'static {
        // float the connection in the pool before we move into the task
        // in case the returned `Future` isn't executed, like if it's spawned into a dying runtime
        // https://github.com/launchbadge/sqlx/issues/1396
        // Type hints seem to be broken by `Option` combinators in IntelliJ Rust right now (6/22).
        let floating: Option<Floating<Live>> =
            self.live.take().map(|live| live.float(self.pool.clone()));

        async move {
            if let Some(floating) = floating {
                floating.return_to_pool().await
            } else {
                false
            };
        }
    }
}

/// Returns the connection to the [`Pool`][crate::Pool] it was checked-out from.
impl Drop for PoolConnection {
    fn drop(&mut self) {
        // We still need to spawn a task to maintain `min_connections`.
        if self.live.is_some()
            && let Ok(handle) = Handle::try_current()
        {
            handle.spawn(self.return_to_pool());
        } else if let Some(live) = self.live.take() {
            let floating = live.float(self.pool.clone());
            if self.pool.is_closed() {
                drop(floating);
            } else {
                floating.release();
            }
        }
    }
}

impl Live {
    /// Convert a live connection into a floating connection.
    pub fn float(self, pool: Arc<PoolInner>) -> Floating<Self> {
        Floating {
            inner: self,
            // create a new guard from a previously leaked permit
            guard: DecrementSizeGuard::new_permit(pool),
        }
    }

    /// Convert a live connection into an idle wrapper.
    pub fn into_idle(self) -> Idle {
        Idle { live: self }
    }
}

impl Deref for Idle {
    type Target = Live;

    fn deref(&self) -> &Self::Target {
        &self.live
    }
}

impl DerefMut for Idle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.live
    }
}

impl Floating<Live> {
    /// Create a new floating live connection.
    pub fn new_live(conn: Connection, guard: DecrementSizeGuard) -> Self {
        Self {
            inner: Live { raw: conn },
            guard,
        }
    }

    /// Reattach the floating connection to the pool wrapper.
    pub fn reattach(self) -> PoolConnection {
        let Self { inner, guard } = self;

        let pool = Arc::clone(&guard.pool);

        guard.cancel();
        PoolConnection {
            live: Some(inner),
            pool,
        }
    }

    /// Release the floating connection back into the pool.
    pub fn release(self) {
        self.guard.pool.clone().release(self);
    }

    /// Return the connection to the pool.
    ///
    /// Returns `true` if the connection was successfully returned, `false` if it was closed.
    async fn return_to_pool(self) -> bool {
        // Immediately close the connection.
        if self.guard.pool.is_closed() {
            self.close().await;
            return false;
        }
        self.release();
        true
    }

    /// Close the underlying connection and drop the size guard.
    pub async fn close(self) {
        // This isn't used anywhere that we care about the return value.
        let _close_result = self.inner.raw.close().await;

        // `guard` is dropped as intended
    }

    /// Convert a floating live connection into a floating idle connection.
    pub fn into_idle(self) -> Floating<Idle> {
        Floating {
            inner: self.inner.into_idle(),
            guard: self.guard,
        }
    }
}

impl Floating<Idle> {
    /// Create a floating idle connection from an idle connection and a permit.
    pub fn from_idle(idle: Idle, pool: Arc<PoolInner>, permit: SemaphorePermit<'_>) -> Self {
        Self {
            inner: idle,
            guard: DecrementSizeGuard::from_permit(pool, permit),
        }
    }

    /// Convert a floating idle connection back into a floating live connection.
    pub fn into_live(self) -> Floating<Live> {
        Floating {
            inner: self.inner.live,
            guard: self.guard,
        }
    }
}

impl<C> Deref for Floating<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<C> DerefMut for Floating<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
