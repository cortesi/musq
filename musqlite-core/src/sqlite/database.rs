pub(crate) use crate::database::{Database, HasStatementCache};

use crate::sqlite::SqliteQueryResult;

/// Sqlite database driver.
#[derive(Debug)]
pub struct Sqlite;

impl Database for Sqlite {
    type QueryResult = SqliteQueryResult;
}

impl HasStatementCache for Sqlite {}
