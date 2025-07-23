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

/// Marker trait for primitive types that can be encoded by reference
pub trait PrimitiveEncode: Encode + Copy + 'static {}

// Implement PrimitiveEncode for all our primitive types
impl PrimitiveEncode for bool {}
impl PrimitiveEncode for i8 {}
impl PrimitiveEncode for i16 {}
impl PrimitiveEncode for i32 {}
impl PrimitiveEncode for i64 {}
impl PrimitiveEncode for u8 {}
impl PrimitiveEncode for u16 {}
impl PrimitiveEncode for u32 {}
impl PrimitiveEncode for f32 {}
impl PrimitiveEncode for f64 {}

// Blanket implementation for primitive types
impl<T> Encode for &T
where
    T: PrimitiveEncode,
{
    fn encode(self) -> Result<Value, EncodeError> {
        (*self).encode()
    }
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
