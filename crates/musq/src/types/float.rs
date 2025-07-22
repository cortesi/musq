use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

impl Encode for f32 {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Double {
            value: self.into(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for f32 {
    fn decode(value: &'r Value) -> std::result::Result<f32, DecodeError> {
        compatible!(value, SqliteDataType::Float | SqliteDataType::Numeric);
        Ok(value.double()? as f32)
    }
}

impl Encode for f64 {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Double {
            value: self,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for f64 {
    fn decode(value: &'r Value) -> std::result::Result<f64, DecodeError> {
        compatible!(value, SqliteDataType::Float | SqliteDataType::Numeric);
        value.double()
    }
}
