use crate::{sqlite::SqliteDataType, ustr::UStr};

use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct Column {
    pub(crate) name: UStr,
    pub(crate) ordinal: usize,
    pub(crate) type_info: SqliteDataType,
}

impl Column {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn type_info(&self) -> &SqliteDataType {
        &self.type_info
    }
}
