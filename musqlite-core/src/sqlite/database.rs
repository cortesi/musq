pub(crate) use crate::database::{Database, HasStatementCache};

use crate::sqlite::{SqliteConnection, SqliteQueryResult, SqliteRow, SqliteTransactionManager};

/// Sqlite database driver.
#[derive(Debug)]
pub struct Sqlite;

impl Database for Sqlite {
    type Connection = SqliteConnection;

    type TransactionManager = SqliteTransactionManager;

    type Row = SqliteRow;

    type QueryResult = SqliteQueryResult;
}

impl HasStatementCache for Sqlite {}
