use std::borrow::Cow;

use crate::{
    error::BoxDynError, type_info::DataType, types::Type, Sqlite, SqliteArgumentValue,
    SqliteTypeInfo, SqliteValueRef,
};
use sqlx_core::{
    decode::Decode,
    encode::{Encode, IsNull},
};

impl Type<Sqlite> for [u8] {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Blob)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Blob | DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for &'q [u8] {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Blob(Cow::Borrowed(self)));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for &'r [u8] {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.blob())
    }
}

impl Type<Sqlite> for Vec<u8> {
    fn type_info() -> SqliteTypeInfo {
        <&[u8] as Type<Sqlite>>::type_info()
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        <&[u8] as Type<Sqlite>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Sqlite> for Vec<u8> {
    fn encode(self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Blob(Cow::Owned(self)));

        IsNull::No
    }

    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Blob(Cow::Owned(self.clone())));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for Vec<u8> {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(value.blob().to_owned())
    }
}
