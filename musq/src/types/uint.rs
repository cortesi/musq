use crate::{
    compatible,
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
};

impl Encode for u8 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Int(self as i32));
        IsNull::No
    }
}

impl<'r> Decode<'r> for u8 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        Ok(value.int().try_into()?)
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
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        Ok(value.int().try_into()?)
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
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        Ok(value.int64().try_into()?)
    }
}
