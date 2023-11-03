use std::borrow::Cow;

use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
};

use crate::sqlite::{
    error::BoxDynError, type_info::DataType, types::Type, Sqlite, SqliteArgumentValue, TypeInfo,
    ValueRef,
};

impl Type<Sqlite> for str {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for &'q str {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(Cow::Borrowed(*self)));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for &'r str {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        value.text()
    }
}

impl Type<Sqlite> for String {
    fn type_info() -> TypeInfo {
        <&str as Type<Sqlite>>::type_info()
    }
}

impl<'q> Encode<'q, Sqlite> for String {
    fn encode(self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(Cow::Owned(self)));

        IsNull::No
    }

    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(Cow::Owned(self.clone())));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for String {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        value.text().map(ToOwned::to_owned)
    }
}

impl Type<Sqlite> for Cow<'_, str> {
    fn type_info() -> TypeInfo {
        <&str as Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &TypeInfo) -> bool {
        <&str as Type<Sqlite>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Sqlite> for Cow<'q, str> {
    fn encode(self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(self));

        IsNull::No
    }

    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Text(self.clone()));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for Cow<'r, str> {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        value.text().map(Cow::Borrowed)
    }
}
