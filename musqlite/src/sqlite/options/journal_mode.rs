/// Refer to [SQLite documentation] for the meaning of the database journaling mode.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_journal_mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum JournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    #[default]
    Wal,
    Off,
}

impl JournalMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            JournalMode::Delete => "DELETE",
            JournalMode::Truncate => "TRUNCATE",
            JournalMode::Persist => "PERSIST",
            JournalMode::Memory => "MEMORY",
            JournalMode::Wal => "WAL",
            JournalMode::Off => "OFF",
        }
    }
}
