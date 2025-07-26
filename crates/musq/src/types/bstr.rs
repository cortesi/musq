/// Conversions between `bstr` types and SQL types.
use crate::{
    SqliteDataType, Value,
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
};

#[doc(no_inline)]
pub use bstr::{BStr, BString, ByteSlice};

impl<'r> Decode<'r> for BString {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(BString::from(value.blob().to_owned()))
    }
}

impl Encode for &BStr {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Blob {
            value: self.as_bytes().to_vec(),
            type_info: None,
        })
    }
}

impl Encode for BString {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Blob {
            value: self.as_bytes().to_vec(),
            type_info: None,
        })
    }
}
