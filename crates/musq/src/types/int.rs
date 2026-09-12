use std::result::Result as StdResult;

use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

encode_integer!(i8, raw);
decode_integer!(i8, int, narrow);

encode_integer!(i16, raw);
decode_integer!(i16, int, narrow);

encode_integer!(i32, raw);
decode_integer!(i32, int, direct);

encode_integer!(i64, raw);
decode_integer!(i64, int64, direct);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_encode() {
        let value = 42i32;
        let result = value.encode().unwrap();
        if let Value::Integer { value: encoded, .. } = result {
            assert_eq!(encoded, 42);
        } else {
            panic!("Expected Integer value");
        }

        let value_i8 = 127i8;
        let result = value_i8.encode().unwrap();
        if let Value::Integer { value: encoded, .. } = result {
            assert_eq!(encoded, 127);
        } else {
            panic!("Expected Integer value");
        }

        let value_u32 = 123u32;
        let result = value_u32.encode().unwrap();
        if let Value::Integer { value: encoded, .. } = result {
            assert_eq!(encoded, 123);
        } else {
            panic!("Expected Integer value");
        }
    }
}
