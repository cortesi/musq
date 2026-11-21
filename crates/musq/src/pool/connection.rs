use std::{
    fmt::{self, Debug, Formatter},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use tokio::{runtime::Handle, sync::SemaphorePermit};

use super::inner::{DecrementSizeGuard, PoolInner};
use crate::{Connection, Result};

/// A single database connection acquired from a [`crate::Pool`].
///
/// A `PoolConnection` represents exclusive access to a single physical database
/// connection. Its primary purpose is to handle sequences of operations that
/// must run on the same connection but are not part of a formal transaction.
///
/// For most database interactions, it is more convenient to execute queries
/// directly on a a shared [`crate::Pool`] reference or to use a
/// [`crate::Transaction`].
///
/// A `PoolConnection` is automatically returned to the pool when it is dropped.
///
/// ### Use Case: Temporary Tables
///
/// The most common reason to manually acquire a `PoolConnection` is to work
/// with temporary tables, which are only visible to the connection that created
/// them.
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
    /// Underlying connection, if still attached.
    conn: Option<Connection>,
    /// Owning pool reference.
    pool: Arc<PoolInner>,
}

/// RAII wrapper for a connection whose pool slot is tracked by `guard`.
pub(super) struct Floating {
    /// Underlying connection handle.
    pub(super) conn: Connection,
    /// Guard that decrements pool size on drop.
    pub(super) guard: DecrementSizeGuard,
}

/// Error message for missing pooled connection state.
const EXPECT_MSG: &str = "BUG: inner connection already taken!";

impl Debug for PoolConnection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolConnection").finish()
    }
}

impl Deref for PoolConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().expect(EXPECT_MSG)
    }
}

impl DerefMut for PoolConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().expect(EXPECT_MSG)
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
        let conn = self.conn.take().expect(EXPECT_MSG);
        let floating = Floating::new(conn, DecrementSizeGuard::new_permit(Arc::clone(&self.pool)));
        floating.close().await
    }
}

/// Returns the connection to the [`Pool`][crate::Pool] it was checked-out from.
impl Drop for PoolConnection {
    fn drop(&mut self) {
        let Some(conn) = self.conn.take() else {
            return;
        };

        if self.pool.is_closed() {
            let floating =
                Floating::new(conn, DecrementSizeGuard::new_permit(Arc::clone(&self.pool)));
            if let Ok(handle) = Handle::try_current() {
                handle.spawn(async move {
                    let _close_result = floating.close().await;
                });
            } else {
                drop(floating);
            }
        } else {
            self.pool.release_conn(conn);
        }
    }
}

impl Floating {
    /// Create a new floating connection.
    pub(super) fn new(conn: Connection, guard: DecrementSizeGuard) -> Self {
        Self { conn, guard }
    }

    /// Create a floating connection from an idle connection and a permit.
    pub(super) fn from_idle(
        conn: Connection,
        pool: Arc<PoolInner>,
        permit: SemaphorePermit<'_>,
    ) -> Self {
        Self {
            conn,
            guard: DecrementSizeGuard::from_permit(pool, permit),
        }
    }

    /// Reattach the floating connection to a [`PoolConnection`].
    pub(super) fn reattach(self) -> PoolConnection {
        let Self { conn, guard } = self;

        let pool = Arc::clone(&guard.pool);
        guard.cancel();
        PoolConnection {
            conn: Some(conn),
            pool,
        }
    }

    /// Close the underlying connection and drop the size guard.
    pub(super) async fn close(self) -> Result<()> {
        let Self { conn, guard } = self;
        let result = conn.close().await;
        drop(guard);
        result
    }
}
