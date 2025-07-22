use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

impl Encode for bool {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: self.into(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for bool {
    fn decode(value: &'r Value) -> std::result::Result<bool, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Bool
                | SqliteDataType::Int
                | SqliteDataType::Int64
                | SqliteDataType::Numeric
        );
        Ok(value.int()? != 0)
    }
}
