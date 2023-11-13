use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::BoxDynError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
    Type,
};

impl Type for f32 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Float
    }
}

impl<'q> Encode<'q> for f32 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Double((*self).into()));

        IsNull::No
    }
}

impl<'r> Decode<'r> for f32 {
    fn decode(value: &'r Value) -> Result<f32, BoxDynError> {
        Ok(value.double() as f32)
    }
}

impl Type for f64 {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Float
    }
}

impl<'q> Encode<'q> for f64 {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Double(*self));

        IsNull::No
    }
}

impl<'r> Decode<'r> for f64 {
    fn decode(value: &'r Value) -> Result<f64, BoxDynError> {
        Ok(value.double())
    }
}
