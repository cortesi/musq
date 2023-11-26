use std::sync::Arc;

/// Conversions between `bstr` types and SQL types.
use crate::{
    compatible, decode::Decode, encode::Encode, error::DecodeError, ArgumentValue, Result,
    SqliteDataType, Value,
};

#[doc(no_inline)]
pub use bstr::{BStr, BString, ByteSlice};

impl<'r> Decode<'r> for BString {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(BString::from(value.blob().to_owned()))
    }
}

impl<'q> Encode for &'q BStr {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Blob(Arc::new(self.as_bytes().to_owned()))
    }
}

impl Encode for BString {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Blob(Arc::new(self.as_bytes().to_vec()))
    }
}
