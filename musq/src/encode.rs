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
    /// Writes the value of `self` into `buf` in the expected format for the database.
    #[must_use]
    fn encode(self, buf: &mut ArgumentBuffer<'q>) -> IsNull
    where
        Self: Sized,
    {
        self.encode_by_ref(buf)
    }

    /// Writes the value of `self` into `buf` without moving `self`.
    ///
    /// Where possible, make use of `encode` instead as it can take advantage of re-using
    /// memory.
    #[must_use]
    fn encode_by_ref(&self, buf: &mut ArgumentBuffer<'q>) -> IsNull;
}

impl<'q, T> Encode<'q> for &'_ T
where
    T: Encode<'q>,
{
    fn encode(self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        <T as Encode>::encode_by_ref(self, buf)
    }

    fn encode_by_ref(&self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        <&T as Encode>::encode(self, buf)
    }
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

    fn encode_by_ref(&self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        if let Some(v) = self {
            v.encode_by_ref(buf)
        } else {
            IsNull::Yes
        }
    }
}
