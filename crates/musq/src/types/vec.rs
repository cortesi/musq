use std::result::Result as StdResult;

use bytemuck::cast_slice;

use crate::{
    SqliteDataType, Value,
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
};

/// A `float32` vector stored as a BLOB.
///
/// This binds directly as raw bytes and works with sqlite-vec APIs that accept
/// float vectors (the default subtype).
#[cfg_attr(docsrs, doc(cfg(feature = "vec")))]
#[derive(Debug, Clone, PartialEq)]
pub struct VecF32(pub Vec<f32>);

/// An `int8` vector stored as a BLOB.
///
/// sqlite-vec requires an explicit SQL wrapper to set the subtype for int8
/// vectors. Use `vec_int8(?)` around bound parameters.
#[cfg_attr(docsrs, doc(cfg(feature = "vec")))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VecInt8(pub Vec<i8>);

/// A packed bit vector stored as a BLOB.
///
/// sqlite-vec requires an explicit SQL wrapper to set the subtype for bit
/// vectors. Use `vec_bit(?)` around bound parameters.
#[cfg_attr(docsrs, doc(cfg(feature = "vec")))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VecBit(pub Vec<u8>);

impl Encode for VecF32 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Blob {
            value: cast_slice::<f32, u8>(&self.0).to_vec().into(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for VecF32 {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob);
        let bytes = value.blob()?;
        let chunks = bytes.chunks_exact(4);
        if !chunks.remainder().is_empty() {
            return Err(DecodeError::Conversion(format!(
                "invalid float32 blob length {}; expected multiple of 4 bytes",
                bytes.len()
            )));
        }

        let mut out = Vec::with_capacity(bytes.len() / 4);
        for chunk in chunks {
            out.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }

        Ok(Self(out))
    }
}

impl Encode for VecInt8 {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Blob {
            value: cast_slice::<i8, u8>(&self.0).to_vec().into(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for VecInt8 {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob);
        let values = value
            .blob()?
            .iter()
            .copied()
            .map(|b| i8::from_ne_bytes([b]))
            .collect();
        Ok(Self(values))
    }
}

impl Encode for VecBit {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Blob {
            value: self.0.clone().into(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for VecBit {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob);
        Ok(Self(value.blob()?.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_f32_round_trip() {
        let input = VecF32(vec![1.5, -2.25, 7.0]);
        let encoded = input.encode().unwrap();
        let decoded = VecF32::decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn vec_f32_empty_round_trip() {
        let input = VecF32(Vec::new());
        let encoded = input.encode().unwrap();
        let decoded = VecF32::decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn vec_f32_invalid_blob_length() {
        let value = Value::Blob {
            value: vec![1_u8, 2, 3].into(),
            type_info: None,
        };
        let err = VecF32::decode(&value).unwrap_err();
        assert!(matches!(err, DecodeError::Conversion(_)));
    }

    #[test]
    fn vec_int8_full_range_round_trip() {
        let input = VecInt8((i8::MIN..=i8::MAX).collect());
        let encoded = input.encode().unwrap();
        let decoded = VecInt8::decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn vec_bit_round_trip() {
        let input = VecBit(vec![0b1010_1010, 0b1100_0011, 0, 255]);
        let encoded = input.encode().unwrap();
        let decoded = VecBit::decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }
}
