use std::{
    fmt::{self, Debug, Formatter},
    ops::{Deref, DerefMut},
    sync::atomic::Ordering,
};

use futures_core::future::BoxFuture;

use crate::{Connection, PoolConnection, Result};

/// How SQLite starts a top-level transaction.
///
/// Nested [`Transaction::begin`] calls still create savepoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransactionBehavior {
    /// `BEGIN DEFERRED`. Locks are taken when they are needed.
    Deferred,
    /// `BEGIN IMMEDIATE`. A reserved lock is taken at begin.
    ///
    /// This is the default. Most explicit transactions write, and Immediate
    /// avoids `SQLITE_BUSY_SNAPSHOT` when a second connection commits between
    /// a deferred read and a later write.
    #[default]
    Immediate,
    /// `BEGIN EXCLUSIVE`. An exclusive lock is taken at begin.
    Exclusive,
}

/// An in-progress database transaction or savepoint.
///
/// A transaction is a sequence of operations performed as a single logical unit of work. All
/// commands within a transaction are guaranteed to execute on the same database connection.
///
/// A transaction is started by calling [`crate::Pool::begin`] or [`Connection::begin`].
/// Top-level transactions use [`TransactionBehavior::Immediate`] unless a
/// different default or [`Self::begin_with`] is set. It must be concluded by calling
/// either [`commit()`] or [`rollback()`], both of which consume the transaction.
///
/// If a `Transaction` object is dropped without being explicitly committed or rolled back, it
/// will automatically be rolled back.
///
/// ### Savepoints (Nested Transactions)
///
/// A `Transaction` can also represent a savepoint within a larger transaction. Calling `begin()`
/// on an existing `Transaction` will create a new savepoint.
///
/// [`commit()`]: Self::commit()
/// [`rollback()`]: Self::rollback()
pub struct Transaction<C = PoolConnection>
where
    C: DerefMut<Target = Connection> + Send,
{
    /// Underlying connection for the transaction.
    connection: C,
    /// Whether the transaction is still open.
    open: bool,
}

impl<C> Transaction<C>
where
    C: DerefMut<Target = Connection> + Send,
{
    /// Begin a transaction using the connection's default behavior.
    pub fn begin<'c>(conn: C) -> BoxFuture<'c, Result<Self>>
    where
        C: 'c,
    {
        let behavior = conn.deref().default_transaction_behavior;
        Self::begin_with(conn, behavior)
    }

    /// Begin a transaction with an explicit start mode.
    ///
    /// Nested calls still create savepoints. `behavior` applies only when this
    /// is the outer transaction.
    pub fn begin_with<'c>(conn: C, behavior: TransactionBehavior) -> BoxFuture<'c, Result<Self>>
    where
        C: 'c,
    {
        Box::pin(async move {
            conn.deref().worker.begin(behavior).await?;
            Ok(Self {
                connection: conn,
                open: true,
            })
        })
    }

    /// Commits this transaction or savepoint.
    pub async fn commit(mut self) -> Result<()> {
        self.connection.deref().worker.commit().await?;
        self.open = false;
        Ok(())
    }

    /// Aborts this transaction or savepoint.
    pub async fn rollback(mut self) -> Result<()> {
        self.connection.deref().worker.rollback().await?;
        self.open = false;
        Ok(())
    }
}

impl<C> Debug for Transaction<C>
where
    C: DerefMut<Target = Connection> + Debug + Send,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("open", &self.open)
            .field(
                "transaction_depth",
                &self
                    .connection
                    .deref()
                    .worker
                    .shared
                    .transaction_depth
                    .load(Ordering::Acquire),
            )
            .finish()
    }
}

impl<C> Deref for Transaction<C>
where
    C: DerefMut<Target = Connection> + Send,
{
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.connection.deref()
    }
}

impl<C> DerefMut for Transaction<C>
where
    C: DerefMut<Target = Connection> + Send,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.connection.deref_mut()
    }
}

impl<C> Drop for Transaction<C>
where
    C: DerefMut<Target = Connection> + Send,
{
    fn drop(&mut self) {
        if self.open {
            self.connection.deref().worker.start_rollback().ok();
        }
    }
}

/// Build SQL for beginning a transaction or savepoint at `depth`.
pub fn begin_sql(depth: usize, behavior: TransactionBehavior) -> String {
    if depth == 0 {
        match behavior {
            TransactionBehavior::Deferred => "BEGIN DEFERRED".into(),
            TransactionBehavior::Immediate => "BEGIN IMMEDIATE".into(),
            TransactionBehavior::Exclusive => "BEGIN EXCLUSIVE".into(),
        }
    } else {
        format!("SAVEPOINT _musq_savepoint_{depth}")
    }
}

/// Build SQL for committing a transaction or savepoint at `depth`.
pub fn commit_sql(depth: usize) -> String {
    if depth <= 1 {
        "COMMIT".into()
    } else {
        format!("RELEASE SAVEPOINT _musq_savepoint_{}", depth - 1)
    }
}

/// Build SQL for rolling back a transaction or savepoint at `depth`.
pub fn rollback_sql(depth: usize) -> String {
    if depth <= 1 {
        "ROLLBACK".into()
    } else {
        format!(
            "ROLLBACK TO SAVEPOINT _musq_savepoint_{0}; RELEASE SAVEPOINT _musq_savepoint_{0}",
            depth - 1
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{TransactionBehavior, begin_sql, commit_sql, rollback_sql};

    #[test]
    fn begin_sql_uses_behavior_at_depth_zero() {
        assert_eq!(
            begin_sql(0, TransactionBehavior::Deferred),
            "BEGIN DEFERRED"
        );
        assert_eq!(
            begin_sql(0, TransactionBehavior::Immediate),
            "BEGIN IMMEDIATE"
        );
        assert_eq!(
            begin_sql(0, TransactionBehavior::Exclusive),
            "BEGIN EXCLUSIVE"
        );
    }

    #[test]
    fn nested_sql_uses_savepoints() {
        assert_eq!(
            begin_sql(1, TransactionBehavior::Immediate),
            "SAVEPOINT _musq_savepoint_1"
        );
        assert_eq!(commit_sql(1), "COMMIT");
        assert_eq!(commit_sql(2), "RELEASE SAVEPOINT _musq_savepoint_1");
        assert_eq!(rollback_sql(1), "ROLLBACK");
        assert_eq!(
            rollback_sql(2),
            "ROLLBACK TO SAVEPOINT _musq_savepoint_1; RELEASE SAVEPOINT _musq_savepoint_1"
        );
    }
}
