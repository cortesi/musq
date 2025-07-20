use std::sync::Arc;

use crate::{SqliteDataType, Value, decode::Decode, encode::Encode, error::DecodeError};

impl Encode for &[u8] {
    fn encode(self) -> Value {
        Value::Blob {
            value: self.to_owned(),
            type_info: None,
        }
    }
}

impl<'r> Decode<'r> for &'r [u8] {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(value.blob())
    }
}

impl Encode for Vec<u8> {
    fn encode(self) -> Value {
        Value::Blob {
            value: self,
            type_info: None,
        }
    }
}

impl<'r> Decode<'r> for Vec<u8> {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(value.blob().to_owned())
    }
}

impl Encode for Arc<Vec<u8>> {
    fn encode(self) -> Value {
        Value::Blob {
            value: (*self).clone(),
            type_info: None,
        }
    }
}

impl<'r> Decode<'r> for Arc<Vec<u8>> {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(Arc::new(value.blob().to_owned()))
    }
}
