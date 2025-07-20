use crate::{
    compatible,
    decode::Decode,
    encode::Encode,
    error::DecodeError,
    sqlite::{SqliteDataType, Value},
};

impl Encode for bool {
    fn encode(self) -> Value {
        Value::Integer(self.into(), None)
    }
}

impl<'r> Decode<'r> for bool {
    fn decode(value: &'r Value) -> std::result::Result<bool, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Bool | SqliteDataType::Int | SqliteDataType::Int64
        );
        Ok(value.int()? != 0)
    }
}
