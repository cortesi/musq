use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
pub use serde_json::value::RawValue as JsonRawValue;
pub use serde_json::Value as JsonValue;

use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::BoxDynError,
    sqlite,
    sqlite::DataType,
    sqlite::{ArgumentBuffer, ArgumentValue},
    types::Type,
    TypeInfo, ValueRef,
};

/// Json for json and jsonb fields
///
/// Will attempt to cast to type passed in as the generic.
///
/// # Example
///
/// ```no_run
/// # use serde::Deserialize;
/// # use musqlite::types;
/// # use musqlite_macros::*;
/// #[derive(Deserialize)]
/// struct Book {
///   name: String
/// }
///
/// #[derive(FromRow)]
/// struct Author {
///   name: String,
///   books: types::Json<Book>
/// }
/// ```
///
/// Can also be used to turn the json/jsonb into a hashmap
/// ```no_run
/// # use musqlite::types;
/// # use musqlite_macros::*;
/// use std::collections::HashMap;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Book {
///   name: String
/// }
/// #[derive(FromRow)]
/// struct Library {
///   id: String,
///   dewey_decimal: types::Json<HashMap<String, Book>>
/// }
/// ```
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Json<T: ?Sized>(pub T);

impl<T> From<T> for Json<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> AsRef<T> for Json<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for Json<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

const JSON_SERIALIZE_ERR: &str = "failed to encode value as JSON; the most likely cause is \
                                  attempting to serialize a map with a non-string key type";

// UNSTABLE: for driver use only!
#[doc(hidden)]
impl<T: Serialize> Json<T> {
    pub fn encode_to_string(&self) -> String {
        // Encoding is supposed to be infallible so we don't have much choice but to panic here.
        // However, I believe that's the right thing to do anyway as an object being unable
        // to serialize to JSON is likely due to a bug or a malformed datastructure.
        serde_json::to_string(self).expect(JSON_SERIALIZE_ERR)
    }

    pub fn encode_to(&self, buf: &mut Vec<u8>) {
        serde_json::to_writer(buf, self).expect(JSON_SERIALIZE_ERR)
    }
}

// UNSTABLE: for driver use only!
#[doc(hidden)]
impl<'a, T: 'a> Json<T>
where
    T: Deserialize<'a>,
{
    pub fn decode_from_string(s: &'a str) -> Result<Self, BoxDynError> {
        serde_json::from_str(s).map_err(Into::into)
    }

    pub fn decode_from_bytes(bytes: &'a [u8]) -> Result<Self, BoxDynError> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }
}

impl<T> Type for Json<T> {
    fn type_info() -> TypeInfo {
        TypeInfo(DataType::Text)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        <&str as Type>::compatible(ty)
    }
}

impl Type for JsonValue
where
    Json<Self>: Type,
{
    fn type_info() -> sqlite::TypeInfo {
        <Json<Self> as Type>::type_info()
    }

    fn compatible(ty: &sqlite::TypeInfo) -> bool {
        <Json<Self> as Type>::compatible(ty)
    }
}

impl<'q> Encode<'q> for JsonValue
where
    for<'a> Json<&'a Self>: Encode<'q>,
{
    fn encode_by_ref(&self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        <Json<&Self> as Encode<'q>>::encode(Json(self), buf)
    }
}

impl<'r> Decode<'r> for JsonValue
where
    Json<Self>: Decode<'r>,
{
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        <Json<Self> as Decode>::decode(value).map(|item| item.0)
    }
}

impl Type for JsonRawValue
where
    for<'a> Json<&'a Self>: Type,
{
    fn type_info() -> sqlite::TypeInfo {
        <Json<&Self> as Type>::type_info()
    }

    fn compatible(ty: &sqlite::TypeInfo) -> bool {
        <Json<&Self> as Type>::compatible(ty)
    }
}

// We don't have to implement Encode for JsonRawValue because that's covered by the default
// implementation for Encode
impl<'r> Decode<'r> for &'r JsonRawValue
where
    Json<Self>: Decode<'r>,
{
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        <Json<Self> as Decode>::decode(value).map(|item| item.0)
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
