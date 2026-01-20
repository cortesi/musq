use std::{result::Result as StdResult, sync::Arc};

use crate::{
    SqliteDataType, Value,
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
};

impl Encode for &[u8] {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Blob {
            value: self.to_vec(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for &'r [u8] {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        value.blob()
    }
}

impl Encode for Vec<u8> {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Blob {
            value: self.clone(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for Vec<u8> {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(value.blob()?.to_owned())
    }
}

impl Encode for Arc<Vec<u8>> {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Blob {
            value: self.as_ref().clone(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for Arc<Vec<u8>> {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(Self::new(value.blob()?.to_owned()))
    }
}
