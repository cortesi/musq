use crate::{
    compatible,
    decode::Decode,
    encode::Encode,
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
};

impl Encode for f32 {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Double(self.into())
    }
}

impl<'r> Decode<'r> for f32 {
    fn decode(value: &'r Value) -> std::result::Result<f32, DecodeError> {
        compatible!(value, SqliteDataType::Float);
        Ok(value.double()? as f32)
    }
}

impl Encode for f64 {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Double(self)
    }
}

impl<'r> Decode<'r> for f64 {
    fn decode(value: &'r Value) -> std::result::Result<f64, DecodeError> {
        compatible!(value, SqliteDataType::Float);
        value.double()
    }
}
