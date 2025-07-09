use std::sync::Arc;

use crate::{
    ArgumentValue, SqliteDataType, Value, compatible, decode::Decode, encode::Encode,
    error::DecodeError,
};

impl Encode for &[u8] {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Blob(Arc::new(self.to_owned()))
    }
}

impl<'r> Decode<'r> for &'r [u8] {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(value.blob())
    }
}

impl Encode for Vec<u8> {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Blob(Arc::new(self))
    }
}

impl<'r> Decode<'r> for Vec<u8> {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(value.blob().to_owned())
    }
}

impl Encode for Arc<Vec<u8>> {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Blob(self.clone())
    }
}

impl<'r> Decode<'r> for Arc<Vec<u8>> {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(Arc::new(value.blob().to_owned()))
    }
}
