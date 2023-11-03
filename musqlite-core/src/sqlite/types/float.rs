use crate::sqlite::{
    error::BoxDynError, type_info::DataType, types::Type, Sqlite, SqliteArgumentValue,
    SqliteValueRef, TypeInfo,
};
use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
};

impl Type<Sqlite> for f32 {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Float)
    }
}

impl<'q> Encode<'q, Sqlite> for f32 {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Double((*self).into()));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for f32 {
    fn decode(value: SqliteValueRef<'r>) -> Result<f32, BoxDynError> {
        Ok(value.double() as f32)
    }
}

impl Type<Sqlite> for f64 {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Float)
    }
}

impl<'q> Encode<'q, Sqlite> for f64 {
    fn encode_by_ref(&self, args: &mut Vec<SqliteArgumentValue<'q>>) -> IsNull {
        args.push(SqliteArgumentValue::Double(*self));

        IsNull::No
    }
}

impl<'r> Decode<'r, Sqlite> for f64 {
    fn decode(value: SqliteValueRef<'r>) -> Result<f64, BoxDynError> {
        Ok(value.double())
    }
}
