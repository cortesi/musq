use std::sync::Arc;

use crate::{
    SqliteDataType, Value, compatible, decode::Decode, encode::Encode, error::DecodeError,
};

impl Encode for &str {
    fn encode(self) -> Value {
        Value::Text(self.as_bytes().to_vec(), None)
    }
}

impl<'r> Decode<'r> for &'r str {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text()
    }
}

impl Encode for String {
    fn encode(self) -> Value {
        Value::Text(self.into_bytes(), None)
    }
}

impl<'r> Decode<'r> for String {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(ToOwned::to_owned)
    }
}

impl Encode for Arc<String> {
    fn encode(self) -> Value {
        Value::Text((*self).clone().into_bytes(), None)
    }
}

impl<'r> Decode<'r> for Arc<String> {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(|x| Arc::new(x.to_owned()))
    }
}
