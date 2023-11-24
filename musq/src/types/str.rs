use std::borrow::Cow;

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

impl<'q> Encode<'q> for &'q str {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(Cow::Borrowed(&self)));
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

impl<'q> Encode<'q> for String {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(Cow::Owned(self)));

        IsNull::No
    }
}

impl<'r> Decode<'r> for String {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        value.text().map(ToOwned::to_owned)
    }
}

impl Type for Cow<'_, str> {
    fn type_info() -> SqliteDataType {
        <&str as Type>::type_info()
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        <&str as Type>::compatible(ty)
    }
}

impl<'q> Encode<'q> for Cow<'q, str> {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(self));

        IsNull::No
    }
}

impl<'r> Decode<'r> for Cow<'r, str> {
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        value.text().map(Cow::Borrowed)
    }
}
