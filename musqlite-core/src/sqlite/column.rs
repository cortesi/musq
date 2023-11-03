use crate::ext::ustr::UStr;
use crate::sqlite::{Sqlite, TypeInfo};

pub(crate) use crate::column::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SqliteColumn {
    pub(crate) name: UStr,
    pub(crate) ordinal: usize,
    pub(crate) type_info: TypeInfo,
}

impl Column for SqliteColumn {
    type Database = Sqlite;

    fn ordinal(&self) -> usize {
        self.ordinal
    }

    fn name(&self) -> &str {
        &*self.name
    }

    fn type_info(&self) -> &TypeInfo {
        &self.type_info
    }
}
