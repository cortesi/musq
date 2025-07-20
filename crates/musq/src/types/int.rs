use crate::{
    compatible,
    decode::Decode,
    encode::Encode,
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
};

impl Encode for i8 {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Int(self as i32)
    }
}

impl<'r> Decode<'r> for i8 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for i16 {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Int(self as i32)
    }
}

impl<'r> Decode<'r> for i16 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for i32 {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Int(self)
    }
}

impl<'r> Decode<'r> for i32 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        value.int()
    }
}

impl Encode for i64 {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Int64(self)
    }
}

impl<'r> Decode<'r> for i64 {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        Ok(value.int64())
    }
}
