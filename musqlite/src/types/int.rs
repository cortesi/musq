use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    sqlite::{error::BoxDynError, ArgumentValue, DataType, TypeInfo, ValueRef},
    Type,
};

impl Type for i8 {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Int)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, DataType::Int | DataType::Int64)
    }
}

impl<'q> Encode<'q> for i8 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(*self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i8 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int().try_into()?)
    }
}

impl Type for i16 {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Int)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, DataType::Int | DataType::Int64)
    }
}

impl<'q> Encode<'q> for i16 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(*self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i16 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int().try_into()?)
    }
}

impl Type for i32 {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Int)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, DataType::Int | DataType::Int64)
    }
}

impl<'q> Encode<'q> for i32 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(*self));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i32 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int())
    }
}

impl Type for i64 {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Int64)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, DataType::Int | DataType::Int64)
    }
}

impl<'q> Encode<'q> for i64 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int64(*self));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i64 {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.int64())
    }
}