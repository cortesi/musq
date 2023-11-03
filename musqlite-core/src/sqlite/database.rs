pub(crate) use crate::database::{Database, HasStatementCache};

use crate::sqlite::{SqliteConnection, SqliteQueryResult, SqliteTransactionManager};

/// Sqlite database driver.
#[derive(Debug)]
pub struct Sqlite;

impl Database for Sqlite {
    type Connection = SqliteConnection;

    type TransactionManager = SqliteTransactionManager;

    type QueryResult = SqliteQueryResult;
}

impl HasStatementCache for Sqlite {}
