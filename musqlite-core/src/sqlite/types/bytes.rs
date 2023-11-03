use std::borrow::Cow;

use crate::sqlite::{
    error::BoxDynError, type_info::DataType, types::Type, ArgumentValue, Sqlite, TypeInfo, ValueRef,
};
use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
};

impl Type for [u8] {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Blob)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, DataType::Blob | DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for &'q [u8] {
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

impl<'q> Encode<'q, Sqlite> for Vec<u8> {
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
