use std::result::Result as StdResult;

use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

impl Encode for u8 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: *self as i64,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for u8 {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for u16 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: *self as i64,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for u16 {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        let v: i32 = value.int()?;
        Ok(v.try_into()?)
    }
}

impl Encode for u32 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: *self as i64,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for u32 {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        Ok(value.int64()?.try_into()?)
    }
}

impl Encode for u64 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: i64::try_from(*self)
                .map_err(|error| EncodeError::Conversion(error.to_string()))?,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for u64 {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        Ok(value.int64()?.try_into()?)
    }
}

impl Encode for usize {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Integer {
            value: i64::try_from(*self)
                .map_err(|error| EncodeError::Conversion(error.to_string()))?,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for usize {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(
            value,
            SqliteDataType::Int | SqliteDataType::Int64 | SqliteDataType::Numeric
        );
        Ok(value.int64()?.try_into()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_u64_boundaries() {
        assert_eq!(0_u64.encode().unwrap().int64().unwrap(), 0);
        assert_eq!(
            (i64::MAX as u64).encode().unwrap().int64().unwrap(),
            i64::MAX
        );
        assert!((i64::MAX as u64 + 1).encode().is_err());

        let negative = Value::Integer {
            value: -1,
            type_info: None,
        };
        assert!(u64::decode(&negative).is_err());
    }

    #[test]
    fn checked_usize_boundaries() {
        assert_eq!(0_usize.encode().unwrap().int64().unwrap(), 0);
        if usize::BITS > 63 {
            assert_eq!(
                (i64::MAX as usize).encode().unwrap().int64().unwrap(),
                i64::MAX
            );
            assert!((i64::MAX as usize + 1).encode().is_err());
        } else {
            assert_eq!(
                usize::MAX.encode().unwrap().int64().unwrap(),
                usize::MAX as i64
            );
        }

        let negative = Value::Integer {
            value: -1,
            type_info: None,
        };
        assert!(usize::decode(&negative).is_err());
    }
}
