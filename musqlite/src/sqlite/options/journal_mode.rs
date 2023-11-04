/// Refer to [SQLite documentation] for the meaning of the database journaling mode.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_journal_mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqliteJournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    Wal,
    Off,
}

impl SqliteJournalMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SqliteJournalMode::Delete => "DELETE",
            SqliteJournalMode::Truncate => "TRUNCATE",
            SqliteJournalMode::Persist => "PERSIST",
            SqliteJournalMode::Memory => "MEMORY",
            SqliteJournalMode::Wal => "WAL",
            SqliteJournalMode::Off => "OFF",
        }
    }
}

impl Default for SqliteJournalMode {
    fn default() -> Self {
        SqliteJournalMode::Wal
    }
}
