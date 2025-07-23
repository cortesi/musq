use std::{
    fmt::{self, Debug, Formatter},
    ops::{Deref, DerefMut},
};

use futures_core::future::BoxFuture;

use crate::{Connection, Result, executor::Executor};

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
/// [`Pool::begin`]: crate::Pool::begin()
/// [`commit`]: Self::commit()
/// [`rollback`]: Self::rollback()
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

impl<'c, C> Executor<'c> for Transaction<C>
where
    C: DerefMut<Target = Connection> + Send,
{
    fn execute<'q, E>(&'c self, query: E) -> BoxFuture<'q, crate::Result<crate::QueryResult>>
    where
        'c: 'q,
        E: crate::executor::Execute + 'q,
    {
        let conn: &'c Connection = self.deref();
        conn.execute(query)
    }

    fn fetch_many<'q, E>(
        &'c self,
        query: E,
    ) -> futures_core::stream::BoxStream<
        'q,
        crate::Result<either::Either<crate::QueryResult, crate::Row>>,
    >
    where
        'c: 'q,
        E: crate::executor::Execute + 'q,
    {
        let conn: &'c Connection = self.deref();
        conn.fetch_many(query)
    }

    fn fetch_optional<'q, E>(
        &'c self,
        query: E,
    ) -> BoxFuture<'q, crate::Result<Option<crate::Row>>>
    where
        'c: 'q,
        E: crate::executor::Execute + 'q,
    {
        let conn: &'c Connection = self.deref();
        conn.fetch_optional(query)
    }

    fn prepare_with<'q>(
        &'c self,
        sql: &'q str,
    ) -> BoxFuture<'q, crate::Result<crate::sqlite::statement::Prepared>>
    where
        'c: 'q,
    {
        let conn: &'c Connection = self.deref();
        conn.prepare_with(sql)
    }
}
