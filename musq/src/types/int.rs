use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
    Type,
};

impl Type for i8 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Int
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        matches!(ty, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl<'q> Encode<'q> for i8 {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i8 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        Ok(value.int().try_into()?)
    }
}

impl Type for i16 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Int
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        matches!(ty, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl<'q> Encode<'q> for i16 {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i16 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        Ok(value.int().try_into()?)
    }
}

impl Type for i32 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Int
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        matches!(ty, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl<'q> Encode<'q> for i32 {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(self));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i32 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        Ok(value.int())
    }
}

impl Type for i64 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Int64
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        matches!(ty, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl<'q> Encode<'q> for i64 {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int64(self));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i64 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        Ok(value.int64())
    }
}
