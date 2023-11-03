use crate::sqlite::{
    error::BoxDynError, type_info::DataType, types::Type, ArgumentValue, Sqlite, TypeInfo, ValueRef,
};
use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
};
use std::borrow::Cow;
use uuid::{
    fmt::{Hyphenated, Simple},
    Uuid,
};

impl Type<Sqlite> for Uuid {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Blob)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, DataType::Blob | DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for Uuid {
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

impl Type<Sqlite> for Hyphenated {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for Hyphenated {
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

impl Type<Sqlite> for Simple {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Text)
    }
}

impl<'q> Encode<'q, Sqlite> for Simple {
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
