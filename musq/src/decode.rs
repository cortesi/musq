//! Provides [`Decode`] for decoding values from the database.
use crate::{error::BoxDynError, ValueRef};

/// A type that can be decoded from the database.
pub trait Decode<'r>: Sized {
    /// Decode a new value of this type using a raw value from the database.
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError>;
}

// implement `Decode` for Option<T> for all SQL types
impl<'r, T> Decode<'r> for Option<T>
where
    T: Decode<'r>,
{
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::decode(value)?))
        }
    }
}
