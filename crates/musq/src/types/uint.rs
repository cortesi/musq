use crate::{
    compatible,
    decode::Decode,
    encode::Encode,
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
};

impl Encode for u8 {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Int(self as i32)
    }
}

impl<'r> Decode<'r> for u8 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for u16 {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Int(self as i32)
    }
}

impl<'r> Decode<'r> for u16 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for u32 {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Int64(self as i64)
    }
}

impl<'r> Decode<'r> for u32 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Int | SqliteDataType::Int64);
        Ok(value.int64()?.try_into()?)
    }
}
