use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::DecodeError,
    sqlite::{ArgumentValue, SqliteDataType, Value},
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
    fn encode(self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Int(self.into()));

        IsNull::No
    }
}

impl<'r> Decode<'r> for bool {
    fn decode(value: &'r Value) -> Result<bool, DecodeError> {
        Ok(value.int() != 0)
    }
}
