use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

impl Encode for bool {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: (*self).into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_encode() {
        let value = true;
        let result = value.encode().unwrap();
        if let Value::Integer { value: encoded, .. } = result {
            assert_eq!(encoded, 1);
        } else {
            panic!("Expected Integer value");
        }

        let value_false = false;
        let result = value_false.encode().unwrap();
        if let Value::Integer { value: encoded, .. } = result {
            assert_eq!(encoded, 0);
        } else {
            panic!("Expected Integer value");
        }
    }
}
