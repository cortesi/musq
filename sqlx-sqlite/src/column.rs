use crate::{Sqlite, SqliteTypeInfo};
use sqlx_core::ext::ustr::UStr;

pub(crate) use sqlx_core::column::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SqliteColumn {
    pub(crate) name: UStr,
    pub(crate) ordinal: usize,
    pub(crate) type_info: SqliteTypeInfo,
}

impl Column for SqliteColumn {
    type Database = Sqlite;

    fn ordinal(&self) -> usize {
        self.ordinal
    }

    fn name(&self) -> &str {
        &*self.name
    }

    fn type_info(&self) -> &SqliteTypeInfo {
        &self.type_info
    }
}
