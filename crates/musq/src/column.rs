use std::fmt::Debug;

use crate::sqlite::SqliteDataType;

/// Metadata about a result column.
#[derive(Debug, Clone)]
pub struct Column {
    /// Declared or inferred SQLite type information.
    pub(crate) type_info: SqliteDataType,
}
