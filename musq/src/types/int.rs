use crate::{
    compatible,
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
};

impl Encode for i8 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Int(self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i8 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        Ok(value.int().try_into()?)
    }
}

impl Encode for i16 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Int(self as i32));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i16 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        Ok(value.int().try_into()?)
    }
}

impl Encode for i32 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Int(self));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i32 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        Ok(value.int())
    }
}

impl Encode for i64 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Int64(self));

        IsNull::No
    }
}

impl<'r> Decode<'r> for i64 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        Ok(value.int64())
    }
}
