use std::result::Result as StdResult;

use crate::{
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::{SqliteDataType, Value},
};

encode_integer!(u8, raw);
decode_integer!(u8, int, narrow);

encode_integer!(u16, raw);
decode_integer!(u16, int, narrow);

encode_integer!(u32, raw);
decode_integer!(u32, int64, narrow);

encode_integer!(u64, checked);
decode_integer!(u64, int64, narrow);

encode_integer!(usize, checked);
decode_integer!(usize, int64, narrow);

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
