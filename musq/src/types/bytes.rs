use std::sync::Arc;

use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
    Type,
};

impl Type for [u8] {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Blob
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        matches!(ty, SqliteDataType::Blob | SqliteDataType::Text)
    }
}

impl<'q> Encode for &'q [u8] {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Blob(Arc::new(self.to_owned())));

        IsNull::No
    }
}

impl<'r> Decode<'r> for &'r [u8] {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        Ok(value.blob())
    }
}

impl Type for Vec<u8> {
    fn type_info() -> SqliteDataType {
        <&[u8] as Type>::type_info()
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        <&[u8] as Type>::compatible(ty)
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
        Ok(value.blob().to_owned())
    }
}
