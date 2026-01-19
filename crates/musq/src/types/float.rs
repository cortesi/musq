use std::result::Result as StdResult;

use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

impl Encode for f32 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Double {
            value: (*self).into(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for f32 {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Float | SqliteDataType::Numeric);
        Ok(value.double()? as Self)
    }
}

impl Encode for f64 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Double {
            value: *self,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for f64 {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Float | SqliteDataType::Numeric);
        value.double()
    }
}

#[cfg(test)]
mod tests {
    use std::{f32::consts::PI, f64::consts::E};

    use super::*;

    #[test]
    fn test_reference_encode() {
        let value = PI;
        let result = value.encode().unwrap();
        if let Value::Double { value: encoded, .. } = result {
            assert!((encoded - value as f64).abs() < 1e-6);
        } else {
            panic!("Expected Double value");
        }

        let value_f64 = E;
        let result = value_f64.encode().unwrap();
        if let Value::Double { value: encoded, .. } = result {
            assert!((encoded - E).abs() < f64::EPSILON);
        } else {
            panic!("Expected Double value");
        }
    }
}
