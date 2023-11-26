use std::sync::Arc;

use crate::{
    compatible,
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    ArgumentValue, SqliteDataType, Value,
};

impl<'q> Encode for &'q str {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Text(Arc::new(self.to_owned())));
        IsNull::No
    }
}

impl<'r> Decode<'r> for &'r str {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text()
    }
}

impl Encode for String {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Text(Arc::new(self)));
        IsNull::No
    }
}

impl<'r> Decode<'r> for String {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(ToOwned::to_owned)
    }
}

impl Encode for Arc<String> {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Text(self.clone()));
        IsNull::No
    }
}

impl<'r> Decode<'r> for Arc<String> {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        compatible!(value, SqliteDataType::Text);
        value.text().map(|x| Arc::new(x.to_owned()))
    }
}
