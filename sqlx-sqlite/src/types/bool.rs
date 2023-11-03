use crate::{
    error::BoxDynError, type_info::DataType, types::Type, Sqlite, SqliteArgumentValue,
    SqliteTypeInfo, SqliteValueRef,
};
use sqlx_core::{
    decode::Decode,
    encode::{Encode, IsNull},
};

impl Type<Sqlite> for bool {
    fn type_info() -> SqliteTypeInfo {
        SqliteTypeInfo(DataType::Bool)
    }

    fn compatible(ty: &SqliteTypeInfo) -> bool {
        matches!(ty.0, DataType::Bool | DataType::Int | DataType::Int64)
    }
}

impl<'q> Encode<'q, Sqlite> for bool {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Int((*self).into()));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for bool {
    fn decode(value: SqliteValueRef<'r>) -> Result<bool, BoxDynError> {
        Ok(value.int() != 0)
    }
}
