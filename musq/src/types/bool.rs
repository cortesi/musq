use crate::{
    compatible,
    decode::Decode,
    encode::Encode,
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
};

impl Encode for bool {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Int(self.into())
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
