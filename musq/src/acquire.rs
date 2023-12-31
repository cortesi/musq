use crate::{
    error::Error,
    pool::{MaybePoolConnection, Pool, PoolConnection},
    Connection,
};

use crate::transaction::Transaction;
use futures_core::future::BoxFuture;
use std::ops::{Deref, DerefMut};

/// Acquire connections or transactions from a database in a generic way.
///
/// The downside of this approach is that you have to `acquire` a connection
/// from a pool first and can't directly pass the pool as argument.
///
/// [workaround]: https://github.com/launchbadge/sqlx/issues/1015#issuecomment-767787777
pub trait Acquire<'c> {
    type Connection: Deref<Target = Connection> + DerefMut + Send;

    fn acquire(self) -> BoxFuture<'c, Result<Self::Connection, Error>>;

    fn begin(self) -> BoxFuture<'c, Result<Transaction<'c>, Error>>;
}

impl<'a> Acquire<'a> for &'_ Pool {
    type Connection = PoolConnection;

    fn acquire(self) -> BoxFuture<'static, Result<Self::Connection, Error>> {
        Box::pin(self.acquire())
    }

    fn begin(self) -> BoxFuture<'static, Result<Transaction<'a>, Error>> {
        let conn = self.acquire();

        Box::pin(async move {
            Transaction::begin(MaybePoolConnection::PoolConnection(conn.await?)).await
        })
    }
}
