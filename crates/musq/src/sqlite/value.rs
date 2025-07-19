use std::str::from_utf8;

use crate::{error::DecodeError, sqlite::type_info::SqliteDataType};

#[derive(Clone)]
pub struct Value {
    pub(crate) data: ValueData,
    pub(crate) type_info: SqliteDataType,
}

#[derive(Clone)]
pub(crate) enum ValueData {
    Null,
    Integer(i64),
    Double(f64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}

impl Value {
    pub fn int(&self) -> i32 {
        self.int64() as i32
    }

    pub fn int64(&self) -> i64 {
        match self.data {
            ValueData::Integer(v) => v,
            _ => 0,
        }
    }

    pub fn double(&self) -> f64 {
        match self.data {
            ValueData::Double(v) => v,
            ValueData::Integer(v) => v as f64,
            _ => 0.0,
        }
    }

    pub fn blob(&self) -> &[u8] {
        match &self.data {
            ValueData::Blob(v) | ValueData::Text(v) => v.as_slice(),
            _ => &[],
        }
    }

    pub fn text(&self) -> Result<&str, DecodeError> {
        match &self.data {
            ValueData::Text(v) => from_utf8(v).map_err(|e| DecodeError::Conversion(e.to_string())),
            _ => Err(DecodeError::Conversion("not text".into())),
        }
    }

    fn type_info_opt(&self) -> Option<SqliteDataType> {
        match self.data {
            ValueData::Null => None,
            ValueData::Integer(_) => Some(SqliteDataType::Int),
            ValueData::Double(_) => Some(SqliteDataType::Float),
            ValueData::Text(_) => Some(SqliteDataType::Text),
            ValueData::Blob(_) => Some(SqliteDataType::Blob),
        }
    }

    pub fn type_info(&self) -> SqliteDataType {
        self.type_info_opt().unwrap_or(self.type_info)
    }

    pub fn is_null(&self) -> bool {
        matches!(self.data, ValueData::Null)
    }
}
