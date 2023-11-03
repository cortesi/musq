use std::borrow::Cow;
use std::fmt::{self, Debug, Formatter};
use std::ops::{Deref, DerefMut};

use futures_core::future::BoxFuture;

use crate::{error::Error, pool::MaybePoolConnection, Connection, TransactionManager};

/// An in-progress database transaction or savepoint.
///
/// A transaction starts with a call to [`Pool::begin`] or [`Connection::begin`].
///
/// A transaction should end with a call to [`commit`] or [`rollback`]. If neither are called
/// before the transaction goes out-of-scope, [`rollback`] is called. In other
/// words, [`rollback`] is called on `drop` if the transaction is still in-progress.
///
/// A savepoint is a special mark inside a transaction that allows all commands that are
/// executed after it was established to be rolled back, restoring the transaction state to
/// what it was at the time of the savepoint.
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
    #[doc(hidden)]
    pub fn begin(conn: impl Into<MaybePoolConnection<'c>>) -> BoxFuture<'c, Result<Self, Error>> {
        let mut conn = conn.into();

        Box::pin(async move {
            TransactionManager::begin(&mut conn).await?;

            Ok(Self {
                connection: conn,
                open: true,
            })
        })
    }

    /// Commits this transaction or savepoint.
    pub async fn commit(mut self) -> Result<(), Error> {
        TransactionManager::commit(&mut self.connection).await?;
        self.open = false;

        Ok(())
    }

    /// Aborts this transaction or savepoint.
    pub async fn rollback(mut self) -> Result<(), Error> {
        TransactionManager::rollback(&mut self.connection).await?;
        self.open = false;

        Ok(())
    }
}

// NOTE: fails to compile due to lack of lazy normalization
// impl<'c, 't, DB: Database> crate::executor::Executor<'t>
//     for &'t mut crate::transaction::Transaction<'c, DB>
// where
//     &'c mut DB::Connection: Executor<'c, Database = DB>,
// {
//     type Database = DB;
//
//
//
//     fn fetch_many<'e, 'q: 'e, E: 'q>(
//         self,
//         query: E,
//     ) -> futures_core::stream::BoxStream<
//         'e,
//         Result<
//             crate::Either<<DB as crate::database::Database>::QueryResult, DB::Row>,
//             crate::error::Error,
//         >,
//     >
//     where
//         't: 'e,
//         E: crate::executor::Execute<'q, Self::Database>,
//     {
//         (&mut **self).fetch_many(query)
//     }
//
//     fn fetch_optional<'e, 'q: 'e, E: 'q>(
//         self,
//         query: E,
//     ) -> futures_core::future::BoxFuture<'e, Result<Option<DB::Row>, crate::error::Error>>
//     where
//         't: 'e,
//         E: crate::executor::Execute<'q, Self::Database>,
//     {
//         (&mut **self).fetch_optional(query)
//     }
//
//     fn prepare_with<'e, 'q: 'e>(
//         self,
//         sql: &'q str,
//         parameters: &'e [<Self::Database as crate::database::Database>::TypeInfo],
//     ) -> futures_core::future::BoxFuture<
//         'e,
//         Result<
//             <Self::Database as crate::database::HasStatement<'q>>::Statement,
//             crate::error::Error,
//         >,
//     >
//     where
//         't: 'e,
//     {
//         (&mut **self).prepare_with(sql, parameters)
//     }
//
//     #[doc(hidden)]
//     fn describe<'e, 'q: 'e>(
//         self,
//         query: &'q str,
//     ) -> futures_core::future::BoxFuture<
//         'e,
//         Result<crate::describe::Describe<Self::Database>, crate::error::Error>,
//     >
//     where
//         't: 'e,
//     {
//         (&mut **self).describe(query)
//     }
// }

impl<'c> Debug for Transaction<'c> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // TODO: Show the full type <..<..<..
        f.debug_struct("Transaction").finish()
    }
}

impl<'c> Deref for Transaction<'c> {
    type Target = Connection;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl<'c> DerefMut for Transaction<'c> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

// Implement `AsMut<DB::Connection>` so `Transaction` can be given to a
// `PgAdvisoryLockGuard`.
//
// See: https://github.com/launchbadge/sqlx/issues/2520
impl<'c> AsMut<Connection> for Transaction<'c> {
    fn as_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }
}

impl<'c, 't> crate::acquire::Acquire<'t> for &'t mut Transaction<'c> {
    type Connection = &'t mut Connection;

    #[inline]
    fn acquire(self) -> BoxFuture<'t, Result<Self::Connection, Error>> {
        Box::pin(futures_util::future::ok(&mut **self))
    }

    #[inline]
    fn begin(self) -> BoxFuture<'t, Result<Transaction<'t>, Error>> {
        Transaction::begin(&mut **self)
    }
}

impl<'c> Drop for Transaction<'c> {
    fn drop(&mut self) {
        if self.open {
            // starts a rollback operation

            // what this does depends on the database but generally this means we queue a rollback
            // operation that will happen on the next asynchronous invocation of the underlying
            // connection (including if the connection is returned to a pool)

            TransactionManager::start_rollback(&mut self.connection);
        }
    }
}

pub fn begin_ansi_transaction_sql(depth: usize) -> Cow<'static, str> {
    if depth == 0 {
        Cow::Borrowed("BEGIN")
    } else {
        Cow::Owned(format!("SAVEPOINT _sqlx_savepoint_{}", depth))
    }
}

pub fn commit_ansi_transaction_sql(depth: usize) -> Cow<'static, str> {
    if depth == 1 {
        Cow::Borrowed("COMMIT")
    } else {
        Cow::Owned(format!("RELEASE SAVEPOINT _sqlx_savepoint_{}", depth - 1))
    }
}

pub fn rollback_ansi_transaction_sql(depth: usize) -> Cow<'static, str> {
    if depth == 1 {
        Cow::Borrowed("ROLLBACK")
    } else {
        Cow::Owned(format!(
            "ROLLBACK TO SAVEPOINT _sqlx_savepoint_{}",
            depth - 1
        ))
    }
}
