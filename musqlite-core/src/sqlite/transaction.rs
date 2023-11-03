use futures_core::future::BoxFuture;

use crate::error::Error;
use crate::sqlite::Connection;

/// Implementation of [`TransactionManager`] for SQLite.
pub struct TransactionManager;

impl TransactionManager {
    pub fn begin(conn: &mut Connection) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(conn.worker.begin())
    }

    pub fn commit(conn: &mut Connection) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(conn.worker.commit())
    }

    pub fn rollback(conn: &mut Connection) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(conn.worker.rollback())
    }

    pub fn start_rollback(conn: &mut Connection) {
        conn.worker.start_rollback().ok();
    }
}
