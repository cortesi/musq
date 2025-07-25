use std::{
    fmt::{self, Debug, Formatter},
    ops::{Deref, DerefMut},
};

use futures_core::future::BoxFuture;

use crate::{Connection, Result};

/// An in-progress database transaction or savepoint.
///
/// A transaction is a sequence of operations performed as a single logical unit of work. All
/// commands within a transaction are guaranteed to execute on the same database connection.
///
/// A transaction is started by calling [`Pool::begin()`] or [`Connection::begin()`]. It must be
/// concluded by calling either [`commit()`] or [`rollback()`].
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
pub struct Transaction<C>
where
    C: DerefMut<Target = Connection> + Send,
{
    connection: C,
    open: bool,
}

impl<C> Transaction<C>
where
    C: DerefMut<Target = Connection> + Send,
{
    /// Begin a nested transaction
    pub fn begin<'c>(conn: C) -> BoxFuture<'c, Result<Self>>
    where
        C: 'c,
    {
        Box::pin(async move {
            conn.deref().worker.begin().await?;
            Ok(Self {
                connection: conn,
                open: true,
            })
        })
    }

    /// Commits this transaction or savepoint.
    pub async fn commit(&mut self) -> Result<()> {
        self.connection.deref().worker.commit().await?;
        self.open = false;
        Ok(())
    }

    /// Aborts this transaction or savepoint.
    pub async fn rollback(&mut self) -> Result<()> {
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
        // Try to read the current transaction depth. If the lock is currently
        // held elsewhere, we simply omit the value.
        let depth = self
            .connection
            .deref()
            .worker
            .shared
            .conn
            .try_lock()
            .map(|guard| guard.transaction_depth);

        let mut debug = f.debug_struct("Transaction");
        debug.field("open", &self.open);

        match depth {
            Ok(depth) => debug.field("transaction_depth", &depth),
            Err(_) => debug.field("transaction_depth", &"<locked>"),
        };

        debug.finish()
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

pub(crate) fn begin_ansi_transaction_sql(depth: usize) -> String {
    // The first savepoint is equivalent to a BEGIN
    format!("SAVEPOINT _musq_savepoint_{depth}")
}

pub(crate) fn commit_ansi_transaction_sql(depth: usize) -> String {
    format!("RELEASE SAVEPOINT _musq_savepoint_{}", depth - 1)
}

pub(crate) fn rollback_ansi_transaction_sql(depth: usize) -> String {
    if depth == 1 {
        "ROLLBACK".into()
    } else {
        format!("ROLLBACK TO SAVEPOINT _musq_savepoint_{}", depth - 1)
    }
}
