//! Provides [`Decode`] for decoding values from the database.
use crate::{error::DecodeError, Result, Value};

/// A type that can be decoded from the database.
pub trait Decode<'r>: Sized {
    /// Decode a new value of this type using a raw value from the database.
    fn decode(value: &'r Value) -> Result<Self, DecodeError>;
}


// implement `Decode` for Option<T> for all SQL types
impl<'r, T> Decode<'r> for Option<T>
where
    T: Decode<'r>,
{
    fn decode(value: &'r Value) -> Result<Self, DecodeError> {
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::decode(value)?))
        }
    }
}
