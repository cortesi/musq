use std::str::from_utf8;

use crate::{error::DecodeError, sqlite::type_info::SqliteDataType};

/// A database value plus optional type information.
#[derive(Clone, Debug)]
pub enum Value {
    Null(Option<SqliteDataType>),
    Integer(i64, Option<SqliteDataType>),
    Double(f64, Option<SqliteDataType>),
    Text(Vec<u8>, Option<SqliteDataType>),
    Blob(Vec<u8>, Option<SqliteDataType>),
}

impl Value {
    pub fn int(&self) -> std::result::Result<i32, DecodeError> {
        Ok(i32::try_from(self.int64()?)?)
    }

    pub fn int64(&self) -> std::result::Result<i64, DecodeError> {
        match self {
            Value::Integer(v, _) => Ok(*v),
            _ => Err(DecodeError::Conversion("not an integer".into())),
        }
    }

    pub fn double(&self) -> std::result::Result<f64, DecodeError> {
        match self {
            Value::Double(v, _) => Ok(*v),
            Value::Integer(v, _) => Ok(*v as f64),
            _ => Err(DecodeError::Conversion("not a float".into())),
        }
    }

    pub fn blob(&self) -> &[u8] {
        match self {
            Value::Blob(v, _) | Value::Text(v, _) => v.as_slice(),
            _ => &[],
        }
    }

    pub fn text(&self) -> std::result::Result<&str, DecodeError> {
        match self {
            Value::Text(v, _) => from_utf8(v).map_err(|e| DecodeError::Conversion(e.to_string())),
            _ => Err(DecodeError::Conversion("not text".into())),
        }
    }

    pub fn type_info(&self) -> SqliteDataType {
        match self {
            Value::Null(t) => t.unwrap_or(SqliteDataType::Null),
            Value::Integer(_, t) => t.unwrap_or(SqliteDataType::Int),
            Value::Double(_, t) => t.unwrap_or(SqliteDataType::Float),
            Value::Text(_, t) => t.unwrap_or(SqliteDataType::Text),
            Value::Blob(_, t) => t.unwrap_or(SqliteDataType::Blob),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null(_))
    }
}
