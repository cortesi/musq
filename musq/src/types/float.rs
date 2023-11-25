use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
    Type,
};

impl Type for f32 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Float
    }
}

impl Encode for f32 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Double(self.into()));

        IsNull::No
    }
}

impl<'r> Decode<'r> for f32 {
    fn decode(value: &'r Value) -> Result<f32, DecodeError> {
        Ok(value.double() as f32)
    }
}

impl Type for f64 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Float
    }
}

impl Encode for f64 {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Double(self));

        IsNull::No
    }
}

impl<'r> Decode<'r> for f64 {
    fn decode(value: &'r Value) -> Result<f64, DecodeError> {
        Ok(value.double())
    }
}
