//! Provides [`Decode`] for decoding values from the database.
use std::result::Result as StdResult;

use crate::{Value, error::DecodeError};

/// A type that can be decoded from the database.
pub trait Decode<'r>: Sized {
    /// Decode a new value of this type using a raw value from the database.
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError>;
}

// implement `Decode` for Option<T> for all SQL types
impl<'r, T> Decode<'r> for Option<T>
where
    T: Decode<'r>,
{
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::decode(value)?))
        }
    }
}
