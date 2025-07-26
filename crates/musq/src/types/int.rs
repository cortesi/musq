use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

impl Encode for i8 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: *self as i64,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for i8 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for i16 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: *self as i64,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for i16 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for i32 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: *self as i64,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for i32 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        value.int()
    }
}

impl Encode for i64 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: *self,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for i64 {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        value.int64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_encode() {
        let value = 42i32;
        let result = (&value).encode().unwrap();
        if let Value::Integer { value: encoded, .. } = result {
            assert_eq!(encoded, 42);
        } else {
            panic!("Expected Integer value");
        }

        let value_i8 = 127i8;
        let result = (&value_i8).encode().unwrap();
        if let Value::Integer { value: encoded, .. } = result {
            assert_eq!(encoded, 127);
        } else {
            panic!("Expected Integer value");
        }

        let value_u32 = 123u32;
        let result = (&value_u32).encode().unwrap();
        if let Value::Integer { value: encoded, .. } = result {
            assert_eq!(encoded, 123);
        } else {
            panic!("Expected Integer value");
        }
    }
}
