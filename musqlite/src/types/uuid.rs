use crate::sqlite::{error::BoxDynError, ArgumentValue, SqliteDataType, TypeInfo, ValueRef};
use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    Type,
};
use std::borrow::Cow;
pub use uuid::{
    fmt::{Hyphenated, Simple},
    Uuid,
};

impl Type for Uuid {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Blob)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, SqliteDataType::Blob | SqliteDataType::Text)
    }
}

impl<'q> Encode<'q> for Uuid {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Blob(Cow::Owned(self.as_bytes().to_vec())));

        IsNull::No
    }
}

impl Decode<'_> for Uuid {
    fn decode(value: ValueRef<'_>) -> Result<Self, BoxDynError> {
        // construct a Uuid from the returned bytes
        Uuid::from_slice(value.blob()).map_err(Into::into)
    }
}

impl Type for Hyphenated {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Text)
    }
}

impl<'q> Encode<'q> for Hyphenated {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(Cow::Owned(self.to_string())));

        IsNull::No
    }
}

impl Decode<'_> for Hyphenated {
    fn decode(value: ValueRef<'_>) -> Result<Self, BoxDynError> {
        let uuid: Result<Uuid, BoxDynError> =
            Uuid::parse_str(&value.text().map(ToOwned::to_owned)?).map_err(Into::into);

        Ok(uuid?.hyphenated())
    }
}

impl Type for Simple {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Text)
    }
}

impl<'q> Encode<'q> for Simple {
    fn encode_by_ref(&self, args: &mut Vec<ArgumentValue<'q>>) -> IsNull {
        args.push(ArgumentValue::Text(Cow::Owned(self.to_string())));

        IsNull::No
    }
}

impl Decode<'_> for Simple {
    fn decode(value: ValueRef<'_>) -> Result<Self, BoxDynError> {
        let uuid: Result<Uuid, BoxDynError> =
            Uuid::parse_str(&value.text().map(ToOwned::to_owned)?).map_err(Into::into);

        Ok(uuid?.simple())
    }
}
