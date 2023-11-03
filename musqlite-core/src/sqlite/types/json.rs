use serde::{Deserialize, Serialize};

use crate::sqlite::{
    error::BoxDynError,
    type_info::DataType,
    types::{Json, Type},
    ArgumentValue, Sqlite, TypeInfo, ValueRef,
};
use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
};

impl<T> Type<Sqlite> for Json<T> {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Text)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        <&str as Type<Sqlite>>::compatible(ty)
    }
}

impl<T> Encode<'_, Sqlite> for Json<T>
where
    T: Serialize,
{
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        Encode::<Sqlite>::encode(self.encode_to_string(), buf)
    }
}

impl<'r, T> Decode<'r, Sqlite> for Json<T>
where
    T: 'r + Deserialize<'r>,
{
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Self::decode_from_string(Decode::<Sqlite>::decode(value)?)
    }
}
