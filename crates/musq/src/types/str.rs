use std::sync::Arc;

use crate::{SqliteDataType, Value, decode::Decode, encode::Encode, error::{DecodeError, EncodeError}};

impl Encode for &str {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Text {
            value: self.to_owned(),
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for &'r str {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text()
    }
}

impl Encode for String {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(Value::Text {
            value: self,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for String {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(ToOwned::to_owned)
    }
}

impl Encode for Arc<String> {
    fn encode(self) -> Result<Value, EncodeError> {
        Ok(match Arc::try_unwrap(self) {
            Ok(s) => Value::Text {
                value: s,
                type_info: None,
            },
            Err(arc) => Value::Text {
                value: arc.as_ref().clone(),
                type_info: None,
            },
        })
    }
}

impl<'r> Decode<'r> for Arc<String> {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(|x| Arc::new(x.to_owned()))
    }
}
