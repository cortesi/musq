use std::{
    fmt::{self, Debug, Formatter},
    ops::{Deref, DerefMut},
};

use futures_core::future::BoxFuture;

use crate::{Connection, Result, pool::MaybePoolConnection};

/// An in-progress database transaction or savepoint.
///
/// A transaction starts with a call to [`Pool::begin`] or [`Connection::begin`].
///
/// A transaction should end with a call to [`commit`] or [`rollback`]. If neither are called before the transaction
/// goes out-of-scope, [`rollback`] is called. In other words, [`rollback`] is called on `drop` if the transaction is
/// still in-progress.
///
/// A savepoint is a special mark inside a transaction that allows all commands that are executed after it was
/// established to be rolled back, restoring the transaction state to what it was at the time of the savepoint.
///
/// [`Connection::begin`]: crate::connection::Connection::begin()
/// [`Pool::begin`]: crate::pool::Pool::begin()
/// [`commit`]: Self::commit()
/// [`rollback`]: Self::rollback()
pub struct Transaction<'c> {
    connection: MaybePoolConnection<'c>,
    open: bool,
}

impl<'c> Transaction<'c> {
    /// Begin a nested transaction
    pub fn begin(conn: impl Into<MaybePoolConnection<'c>>) -> BoxFuture<'c, Result<Self>> {
        let mut conn = conn.into();
        Box::pin(async move {
            Box::pin(conn.worker.begin()).await?;
            Ok(Self {
                connection: conn,
                open: true,
            })
        })
    }

    /// Commits this transaction or savepoint.
    pub async fn commit(mut self) -> Result<()> {
        Box::pin(self.connection.worker.commit()).await?;
        self.open = false;
        Ok(())
    }

    /// Aborts this transaction or savepoint.
    pub async fn rollback(mut self) -> Result<()> {
        Box::pin(self.connection.worker.rollback()).await?;
        self.open = false;
        Ok(())
    }
}

impl<'c> Debug for Transaction<'c> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Try to read the current transaction depth. If the lock is currently
        // held elsewhere, we simply omit the value.
        let depth = self
            .connection
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

impl<'c> Deref for Transaction<'c> {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl<'c> DerefMut for Transaction<'c> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

impl<'c> Drop for Transaction<'c> {
    fn drop(&mut self) {
        if self.open {
            self.connection.worker.start_rollback().ok();
        }
    }
}

pub fn begin_ansi_transaction_sql(depth: usize) -> String {
    // The first savepoint is equivalent to a BEGIN
    format!("SAVEPOINT _musq_savepoint_{depth}")
}

pub fn commit_ansi_transaction_sql(depth: usize) -> String {
    format!("RELEASE SAVEPOINT _musq_savepoint_{}", depth - 1)
}

pub fn rollback_ansi_transaction_sql(depth: usize) -> String {
    if depth == 1 {
        "ROLLBACK".into()
    } else {
        format!("ROLLBACK TO SAVEPOINT _musq_savepoint_{}", depth - 1)
    }
}
