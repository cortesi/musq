use std::sync::Arc;

/// Conversions between `bstr` types and SQL types.
use crate::{
    compatible,
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::ArgumentBuffer,
    ArgumentValue, Result, SqliteDataType, Value,
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
    fn encode(self, buf: &mut ArgumentBuffer) -> IsNull {
        buf.push(ArgumentValue::Blob(Arc::new(self.as_bytes().to_owned())));
        IsNull::No
    }
}

impl Encode for BString {
    fn encode(self, buf: &mut ArgumentBuffer) -> IsNull {
        buf.push(ArgumentValue::Blob(Arc::new(self.as_bytes().to_vec())));
        IsNull::No
    }
}
