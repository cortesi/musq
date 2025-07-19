//! Provides [`Encode`] for encoding values for the database.
use crate::ArgumentValue;

/// Encode a single value to be sent to the database.
pub trait Encode {
    /// Writes the value of `self` into `buf` in the expected format for the database, consuming the value. Encoders are
    /// implemented for reference counted types where a shift in ownership is not wanted.
    #[must_use]
    fn encode(self) -> ArgumentValue
    where
        Self: Sized;
}

impl<T> Encode for Option<T>
where
    T: Encode,
{
    fn encode(self) -> ArgumentValue {
        if let Some(v) = self {
            v.encode()
        } else {
            ArgumentValue::Null
        }
    }
}
