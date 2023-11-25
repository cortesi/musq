use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
    Type,
};

impl Type for u8 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Int
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        matches!(ty, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl Encode for u8 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Int(self as i32));
        IsNull::No
    }
}

impl<'r> Decode<'r> for u8 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        Ok(value.int().try_into()?)
    }
}

impl Type for u16 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Int
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        matches!(ty, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl Encode for u16 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Int(self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r> for u16 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        Ok(value.int().try_into()?)
    }
}

impl Type for u32 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Int64
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        matches!(ty, SqliteDataType::Int | SqliteDataType::Int64)
    }
}

impl Encode for u32 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Int64(self as i64));
        IsNull::No
    }
}

impl<'r> Decode<'r> for u32 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        Ok(value.int64().try_into()?)
    }
}
