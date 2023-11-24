//! Provides [`Encode`] for encoding values for the database.
use crate::{sqlite::ArgumentBuffer, Type};

/// The return type of [Encode::encode].
pub enum IsNull {
    /// The value is null; no data was written.
    Yes,

    /// The value is not null.
    ///
    /// This does not mean that data was written.
    No,
}

/// Encode a single value to be sent to the database.
pub trait Encode<'q> {
    /// Writes the value of `self` into `buf` in the expected format for the database, consuming the value. Encoders are
    /// implemented for reference counted types where a shift in ownership is not wanted.
    #[must_use]
    fn encode(self, buf: &mut ArgumentBuffer<'q>) -> IsNull
    where
        Self: Sized;
}

impl<'q, T> Encode<'q> for Option<T>
where
    T: Encode<'q> + Type + 'q,
{
    fn encode(self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        if let Some(v) = self {
            v.encode(buf)
        } else {
            IsNull::Yes
        }
    }
}
