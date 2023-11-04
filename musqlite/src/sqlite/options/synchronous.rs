/// Refer to [SQLite documentation] for the meaning of various synchronous settings.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_synchronous
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Synchronous {
    Off,
    Normal,
    Full,
    Extra,
}

impl Synchronous {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Synchronous::Off => "OFF",
            Synchronous::Normal => "NORMAL",
            Synchronous::Full => "FULL",
            Synchronous::Extra => "EXTRA",
        }
    }
}

impl Default for Synchronous {
    fn default() -> Self {
        Synchronous::Full
    }
}
