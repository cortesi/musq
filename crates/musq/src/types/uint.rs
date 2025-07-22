use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

impl Encode for u8 {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: self as i64,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for u8 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for u16 {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: self as i64,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for u16 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for u32 {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: self as i64,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for u32 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        Ok(value.int64()?.try_into()?)
    }
}
