use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    sqlite::{error::BoxDynError, ArgumentValue, SqliteDataType, ValueRef},
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
    fn decode(value: ValueRef<'r>) -> Result<f32, BoxDynError> {
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
    fn decode(value: ValueRef<'r>) -> Result<f64, BoxDynError> {
        Ok(value.double())
    }
}
