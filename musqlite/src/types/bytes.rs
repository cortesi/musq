use std::borrow::Cow;

use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    sqlite::{error::BoxDynError, ArgumentValue, SqliteDataType, TypeInfo, ValueRef},
    Type,
};

impl Type for [u8] {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Blob)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, SqliteDataType::Blob | SqliteDataType::Text)
    }
}

impl<'q> Encode<'q> for &'q [u8] {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Blob(Cow::Borrowed(self)));

        IsNull::No
    }
}

impl<'r> Decode<'r> for &'r [u8] {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.blob())
    }
}

impl Type for Vec<u8> {
    fn type_info() -> TypeInfo {
        <&[u8] as Type>::type_info()
    }

    fn compatible(ty: &TypeInfo) -> bool {
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
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.blob().to_owned())
    }
}
