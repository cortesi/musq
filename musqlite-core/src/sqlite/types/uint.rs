use crate::sqlite::error::BoxDynError;
use crate::sqlite::type_info::DataType;
use crate::sqlite::types::Type;
use crate::sqlite::{ArgumentValue, Sqlite, TypeInfo, ValueRef};
use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
};

impl Type<Sqlite> for u8 {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Int)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, DataType::Int | DataType::Int64)
    }
}

impl<'q> Encode<'q, Sqlite> for u8 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(*self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for u8 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int().try_into()?)
    }
}

impl Type<Sqlite> for u16 {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Int)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, DataType::Int | DataType::Int64)
    }
}

impl<'q> Encode<'q, Sqlite> for u16 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(*self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for u16 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int().try_into()?)
    }
}

impl Type<Sqlite> for u32 {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Int64)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, DataType::Int | DataType::Int64)
    }
}

impl<'q> Encode<'q, Sqlite> for u32 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int64(*self as i64));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for u32 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int64().try_into()?)
    }
}
