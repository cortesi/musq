#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqliteAutoVacuum {
    None,
    Full,
    Incremental,
}

impl SqliteAutoVacuum {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SqliteAutoVacuum::None => "NONE",
            SqliteAutoVacuum::Full => "FULL",
            SqliteAutoVacuum::Incremental => "INCREMENTAL",
        }
    }
}

impl Default for SqliteAutoVacuum {
    fn default() -> Self {
        SqliteAutoVacuum::None
    }
}
