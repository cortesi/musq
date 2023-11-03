use std::borrow::Cow;

use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
};

use crate::sqlite::{
    error::BoxDynError, type_info::DataType, types::Type, ArgumentValue, Sqlite, TypeInfo, ValueRef,
};

impl Type for str {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for &'q str {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(Cow::Borrowed(*self)));

        IsNull::No
    }
}

impl<'r> Decode<'r> for &'r str {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        value.text()
    }
}

impl Type for String {
    fn type_info() -> TypeInfo {
        <&str as Type>::type_info()
    }
}

impl<'q> Encode<'q, Sqlite> for String {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(Cow::Owned(self)));

        IsNull::No
    }

    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(Cow::Owned(self.clone())));

        IsNull::No
    }
}

impl<'r> Decode<'r> for String {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        value.text().map(ToOwned::to_owned)
    }
}

impl Type for Cow<'_, str> {
    fn type_info() -> TypeInfo {
        <&str as Type>::type_info()
    }

    fn compatible(ty: &TypeInfo) -> bool {
        <&str as Type>::compatible(ty)
    }
}

impl<'q> Encode<'q, Sqlite> for Cow<'q, str> {
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(self));

        IsNull::No
    }

    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(self.clone()));

        IsNull::No
    }
}

impl<'r> Decode<'r> for Cow<'r, str> {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        value.text().map(Cow::Borrowed)
    }
}
