use std::borrow::Cow;

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

impl<'q> Encode<'q> for &'q [u8] {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Blob(Cow::Borrowed(self)));

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

impl<'q> Encode<'q> for Vec<u8> {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Blob(Cow::Owned(self)));

        IsNull::No
    }

    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Blob(Cow::Owned(self.clone())));

        IsNull::No
    }
}

impl<'r> Decode<'r> for Vec<u8> {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        Ok(value.blob().to_owned())
    }
}
