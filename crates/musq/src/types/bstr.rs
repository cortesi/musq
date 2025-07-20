/// Conversions between `bstr` types and SQL types.
use crate::{
    SqliteDataType, Value, compatible, decode::Decode, encode::Encode, error::DecodeError,
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
    fn encode(self) -> Value {
        Value::Blob(self.as_bytes().to_owned(), None)
    }
}

impl Encode for BString {
    fn encode(self) -> Value {
        Value::Blob(self.as_bytes().to_vec(), None)
    }
}
