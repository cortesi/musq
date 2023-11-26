use crate::{
    compatible,
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
};

impl Encode for bool {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Int(self.into()));

        IsNull::No
    }
}

impl<'r> Decode<'r> for bool {
    fn decode(value: &'r Value) -> Result<bool, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Bool | SqliteDataType::Int | SqliteDataType::Int64
        );
        Ok(value.int() != 0)
    }
}
