use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    sqlite::{error::BoxDynError, ArgumentValue, SqliteDataType, ValueRef},
    Type,
};

impl Type for bool {
    fn type_info() -> SqliteDataType {
        SqliteDataType::Bool
    }

    fn compatible(ty: &SqliteDataType) -> bool {
        matches!(
            ty,
            SqliteDataType::Bool | SqliteDataType::Int | SqliteDataType::Int64
        )
    }
}

impl<'q> Encode<'q> for bool {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int((*self).into()));

        IsNull::No
    }
}

impl<'r> Decode<'r> for bool {
    fn decode(value: ValueRef<'r>) -> Result<bool, BoxDynError> {
        Ok(value.int() != 0)
    }
}
