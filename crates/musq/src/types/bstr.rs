/// Conversions between `bstr` types and SQL types.
use crate::{
    ArgumentValue, Result, SqliteDataType, Value, compatible, decode::Decode, encode::Encode,
    error::DecodeError,
};

#[doc(no_inline)]
pub use bstr::{BStr, BString, ByteSlice};

impl<'r> Decode<'r> for BString {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(BString::from(value.blob().to_owned()))
    }
}

impl Encode for &BStr {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Blob(self.as_bytes().to_owned())
    }
}

impl Encode for BString {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Blob(self.as_bytes().to_vec())
    }
}
