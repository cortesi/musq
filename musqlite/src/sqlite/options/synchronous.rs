/// Refer to [SQLite documentation] for the meaning of various synchronous settings.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_synchronous
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqliteSynchronous {
    Off,
    Normal,
    Full,
    Extra,
}

impl SqliteSynchronous {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SqliteSynchronous::Off => "OFF",
            SqliteSynchronous::Normal => "NORMAL",
            SqliteSynchronous::Full => "FULL",
            SqliteSynchronous::Extra => "EXTRA",
        }
    }
}

impl Default for SqliteSynchronous {
    fn default() -> Self {
        SqliteSynchronous::Full
    }
}
