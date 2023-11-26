use std::sync::Arc;

use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
    Type,
};

// str
impl Type for str {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Text
    }
}

impl<'q> Encode for &'q str {
    fn encode(self, args: &mut Vec<ArgumentValue>) -> IsNull {
        args.push(ArgumentValue::Text(Arc::new(self.to_owned())));
        IsNull::No
    }
}

impl<'r> Decode<'r> for &'r str {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        value.text()
    }
}

// String
impl Type for String {
    fn type_info() -> SqliteDataType {
        <&str as Type>::type_info()
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
        value.text().map(ToOwned::to_owned)
    }
}

// Arc<String>
impl Type for Arc<String> {
    fn type_info() -> SqliteDataType {
        <&str as Type>::type_info()
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
        value.text().map(|x| Arc::new(x.to_owned()))
    }
}
