use serde::{Deserialize, Serialize};

use crate::sqlite::{
    error::BoxDynError,
    type_info::DataType,
    types::{Json, Type},
    ArgumentValue, TypeInfo, ValueRef,
};
use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
};

impl<T> Type for Json<T> {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Text)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        <&str as Type>::compatible(ty)
    }
}

impl<T> Encode<'_> for Json<T>
where
    T: Serialize,
{
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        Encode::encode(self.encode_to_string(), buf)
    }
}

impl<'r, T> Decode<'r> for Json<T>
where
    T: 'r + Deserialize<'r>,
{
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Self::decode_from_string(Decode::decode(value)?)
    }
}
