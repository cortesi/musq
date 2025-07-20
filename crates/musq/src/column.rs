use crate::sqlite::SqliteDataType;

use std::fmt::Debug;

#[derive(Debug, Clone)]
pub(crate) struct Column {
    pub(crate) type_info: SqliteDataType,
}
