use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

impl Encode for f32 {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Double {
            value: self.into(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for f32 {
    fn decode(value: &'r Value) -> std::result::Result<f32, DecodeError> {
        compatible!(value, SqliteDataType::Float | SqliteDataType::Numeric);
        Ok(value.double()? as f32)
    }
}

impl Encode for f64 {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Double {
            value: self,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for f64 {
    fn decode(value: &'r Value) -> std::result::Result<f64, DecodeError> {
        compatible!(value, SqliteDataType::Float | SqliteDataType::Numeric);
        value.double()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_encode() {
        let value = 3.14f32;
        let result = (&value).encode().unwrap();
        if let Value::Double { value: encoded, .. } = result {
            assert!((encoded - value as f64).abs() < 1e-6);
        } else {
            panic!("Expected Double value");
        }

        let value_f64 = 2.718f64;
        let result = (&value_f64).encode().unwrap();
        if let Value::Double { value: encoded, .. } = result {
            assert!((encoded - 2.718).abs() < f64::EPSILON);
        } else {
            panic!("Expected Double value");
        }
    }
}
