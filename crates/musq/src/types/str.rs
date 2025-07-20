use std::sync::Arc;

use crate::{
    ArgumentValue, SqliteDataType, Value, decode::Decode, encode::Encode, error::DecodeError,
};

impl Encode for &str {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Text(self.to_owned())
    }
}

impl<'r> Decode<'r> for &'r str {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text()
    }
}

impl Encode for String {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Text(self)
    }
}

impl<'r> Decode<'r> for String {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(ToOwned::to_owned)
    }
}

impl Encode for Arc<String> {
    fn encode(self) -> ArgumentValue {
        ArgumentValue::Text((*self).clone())
    }
}

impl<'r> Decode<'r> for Arc<String> {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(|x| Arc::new(x.to_owned()))
    }
}
