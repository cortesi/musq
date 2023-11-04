/// Refer to [SQLite documentation] for the meaning of the connection locking mode.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_locking_mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LockingMode {
    #[default]
    Normal,
    Exclusive,
}

impl LockingMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            LockingMode::Normal => "NORMAL",
            LockingMode::Exclusive => "EXCLUSIVE",
        }
    }
}
