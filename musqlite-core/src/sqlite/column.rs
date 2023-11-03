use crate::ext::ustr::UStr;
use crate::sqlite::TypeInfo;

pub(crate) use crate::column::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Column {
    pub(crate) name: UStr,
    pub(crate) ordinal: usize,
    pub(crate) type_info: TypeInfo,
}

impl Column {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn name(&self) -> &str {
        &*self.name
    }

    pub fn type_info(&self) -> &TypeInfo {
        &self.type_info
    }
}
