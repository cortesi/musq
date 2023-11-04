use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    sqlite::{error::BoxDynError, ArgumentValue, SqliteDataType, TypeInfo, ValueRef},
    Type,
};

impl Type for u8 {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Int)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl<'q> Encode<'q> for u8 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(*self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r> for u8 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int().try_into()?)
    }
}

impl Type for u16 {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Int)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl<'q> Encode<'q> for u16 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(*self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r> for u16 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int().try_into()?)
    }
}

impl Type for u32 {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Int64)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl<'q> Encode<'q> for u32 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int64(*self as i64));

        IsNull::No
    }
}

impl<'r> Decode<'r> for u32 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int64().try_into()?)
    }
}
