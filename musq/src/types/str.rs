use std::sync::Arc;

use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
    Type,
};

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

impl Type for String {
    fn type_info() -> SqliteDataType {
        <&str as Type>::type_info()
    }
}

impl<'q> Encode for String {
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
