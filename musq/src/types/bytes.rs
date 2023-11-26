use std::sync::Arc;

use crate::{
    compatible,
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    ArgumentValue, SqliteDataType, Value,
};

impl<'q> Encode for &'q [u8] {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Blob(Arc::new(self.to_owned())));

        IsNull::No
    }
}

impl<'r> Decode<'r> for &'r [u8] {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(value.blob())
    }
}

impl Encode for Vec<u8> {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Blob(Arc::new(self)));

        IsNull::No
    }
}

impl<'r> Decode<'r> for Vec<u8> {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(value.blob().to_owned())
    }
}

impl Encode for Arc<Vec<u8>> {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Blob(self.clone()));

        IsNull::No
    }
}

impl<'r> Decode<'r> for Arc<Vec<u8>> {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(Arc::new(value.blob().to_owned()))
    }
}
