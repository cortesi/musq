//! Provides [`Encode`] for encoding values for the database.
use crate::{Value, error::EncodeError};

/// Encode a single value to be sent to the database.
pub trait Encode {
    /// Writes the value of `self` into `buf` in the expected format for the database, consuming the value. Encoders are
    /// implemented for reference counted types where a shift in ownership is not wanted.
    fn encode(self) -> Result<Value, EncodeError>
    where
        Self: Sized;
}

impl<T> Encode for Option<T>
where
    T: Encode,
{
    fn encode(self) -> Result<Value, EncodeError> {
        if let Some(v) = self {
            v.encode()
        } else {
            Ok(Value::Null { type_info: None })
        }
    }
}
