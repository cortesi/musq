use std::fmt::{self, Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::{Connection, Result, executor::Executor};

use super::inner::{DecrementSizeGuard, PoolInner};
use std::future::Future;

/// A connection managed by a [`Pool`][crate::Pool].
///
/// Will be returned to the pool on-drop.
pub struct PoolConnection {
    live: Option<Live>,
    pool: Arc<PoolInner>,
}

pub(super) struct Live {
    raw: Connection,
}

pub(super) struct Idle {
    pub(super) live: Live,
}

/// RAII wrapper for connections being handled by functions that may drop them
pub(super) struct Floating<C> {
    pub(super) inner: C,
    pub(super) guard: DecrementSizeGuard,
}

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
        if self.live.is_some() {
            tokio::task::spawn(self.return_to_pool());
        }
    }
}

impl Live {
    pub fn float(self, pool: Arc<PoolInner>) -> Floating<Self> {
        Floating {
            inner: self,
            // create a new guard from a previously leaked permit
            guard: DecrementSizeGuard::new_permit(pool),
        }
    }

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
    pub fn new_live(conn: Connection, guard: DecrementSizeGuard) -> Self {
        Self {
            inner: Live { raw: conn },
            guard,
        }
    }

    pub fn reattach(self) -> PoolConnection {
        let Floating { inner, guard } = self;

        let pool = Arc::clone(&guard.pool);

        guard.cancel();
        PoolConnection {
            live: Some(inner),
            pool,
        }
    }

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

    pub async fn close(self) {
        // This isn't used anywhere that we care about the return value
        let _ = self.inner.raw.close().await;

        // `guard` is dropped as intended
    }

    pub fn into_idle(self) -> Floating<Idle> {
        Floating {
            inner: self.inner.into_idle(),
            guard: self.guard,
        }
    }
}

impl Floating<Idle> {
    pub fn from_idle(
        idle: Idle,
        pool: Arc<PoolInner>,
        permit: tokio::sync::SemaphorePermit<'_>,
    ) -> Self {
        Self {
            inner: idle,
            guard: DecrementSizeGuard::from_permit(pool, permit),
        }
    }

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

impl<'c> Executor<'c> for PoolConnection {
    fn execute<'q, E>(
        &'c mut self,
        query: E,
    ) -> futures_core::future::BoxFuture<'q, Result<crate::QueryResult>>
    where
        'c: 'q,
        E: crate::executor::Execute + 'q,
    {
        let conn: &'c mut Connection = self.deref_mut();
        conn.execute(query)
    }

    fn fetch_many<'q, E>(
        &'c mut self,
        query: E,
    ) -> futures_core::stream::BoxStream<'q, Result<either::Either<crate::QueryResult, crate::Row>>>
    where
        'c: 'q,
        E: crate::executor::Execute + 'q,
    {
        let conn: &'c mut Connection = self.deref_mut();
        conn.fetch_many(query)
    }

    fn fetch_optional<'q, E>(
        &'c mut self,
        query: E,
    ) -> futures_core::future::BoxFuture<'q, Result<Option<crate::Row>>>
    where
        'c: 'q,
        E: crate::executor::Execute + 'q,
    {
        let conn: &'c mut Connection = self.deref_mut();
        conn.fetch_optional(query)
    }

    fn prepare_with<'q>(
        &'c mut self,
        sql: &'q str,
    ) -> futures_core::future::BoxFuture<'q, Result<crate::sqlite::statement::Prepared>>
    where
        'c: 'q,
    {
        let conn: &'c mut Connection = self.deref_mut();
        conn.prepare_with(sql)
    }
}
