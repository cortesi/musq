/// Summary information returned after executing a query.
#[derive(Debug, Default)]
pub struct QueryResult {
    /// Number of rows affected by the query.
    pub(super) changes: u64,
    /// Last inserted row ID reported by SQLite.
    pub(super) last_insert_rowid: i64,
}

impl QueryResult {
    /// Return the number of rows affected by the query.
    pub fn rows_affected(&self) -> u64 {
        self.changes
    }

    /// Return the last inserted row ID, if available.
    pub fn last_insert_rowid(&self) -> i64 {
        self.last_insert_rowid
    }
}
