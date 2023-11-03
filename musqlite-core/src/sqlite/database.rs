pub(crate) use crate::database::{Database, HasStatement, HasStatementCache};

use crate::sqlite::{
    SqliteColumn, SqliteConnection, SqliteQueryResult, SqliteRow, SqliteStatement,
    SqliteTransactionManager,
};

/// Sqlite database driver.
#[derive(Debug)]
pub struct Sqlite;

impl Database for Sqlite {
    type Connection = SqliteConnection;

    type TransactionManager = SqliteTransactionManager;

    type Row = SqliteRow;

    type QueryResult = SqliteQueryResult;

    type Column = SqliteColumn;
}

impl<'q> HasStatement<'q> for Sqlite {
    type Database = Sqlite;

    type Statement = SqliteStatement<'q>;
}

impl HasStatementCache for Sqlite {}
