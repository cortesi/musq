pub(crate) use crate::database::{Database, HasStatementCache};

/// Sqlite database driver.
#[derive(Debug)]
pub struct Sqlite;

impl Database for Sqlite {}

impl HasStatementCache for Sqlite {}
