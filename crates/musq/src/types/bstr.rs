use std::result::Result as StdResult;

#[doc(no_inline)]
pub use bstr::{BStr, BString, ByteSlice};

/// Conversions between `bstr` types and SQL types.
use crate::{
    SqliteDataType, Value,
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
};

impl<'r> Decode<'r> for BString {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        compatible!(value, SqliteDataType::Blob | SqliteDataType::Text);
        Ok(Self::from(value.blob()?.to_owned()))
    }
}

impl Encode for BStr {
    fn encode(&self) -> Result<Value, EncodeError> {
        Ok(Value::Blob {
            value: self.as_bytes().to_vec().into(),
            type_info: None,
        })
    }
}

impl Encode for BString {
    fn encode(&self) -> Result<Value, EncodeError> {
        self.as_bstr().encode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_and_owned_bstr_encode() {
        let borrowed = BStr::new(b"borrowed");
        assert_eq!(borrowed.encode().unwrap().blob().unwrap(), b"borrowed");

        let owned = BString::from(Vec::from(&b"owned"[..]));
        assert_eq!(owned.encode().unwrap().blob().unwrap(), b"owned");
    }
}
