#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AutoVacuum {
    #[default]
    None,
    Full,
    Incremental,
}

impl AutoVacuum {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            AutoVacuum::None => "NONE",
            AutoVacuum::Full => "FULL",
            AutoVacuum::Incremental => "INCREMENTAL",
        }
    }
}
