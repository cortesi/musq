//! Provides [`Encode`] for encoding values for the database.

use std::mem;

use crate::{sqlite::ArgumentBuffer, Type, TypeInfo};

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

    fn produces(&self) -> Option<TypeInfo> {
        // `produces` is inherently a hook to allow database drivers to produce value-dependent
        // type information; if the driver doesn't need this, it can leave this as `None`
        None
    }

    #[inline]
    fn size_hint(&self) -> usize {
        mem::size_of_val(self)
    }
}

impl<'q, T> Encode<'q> for &'_ T
where
    T: Encode<'q>,
{
    #[inline]
    fn encode(self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        <T as Encode>::encode_by_ref(self, buf)
    }

    #[inline]
    fn encode_by_ref(&self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        <&T as Encode>::encode(self, buf)
    }

    #[inline]
    fn produces(&self) -> Option<TypeInfo> {
        (**self).produces()
    }

    #[inline]
    fn size_hint(&self) -> usize {
        (**self).size_hint()
    }
}

impl<'q, T> Encode<'q> for Option<T>
where
    T: Encode<'q> + Type + 'q,
{
    #[inline]
    fn produces(&self) -> Option<TypeInfo> {
        if let Some(v) = self {
            v.produces()
        } else {
            T::type_info().into()
        }
    }

    #[inline]
    fn encode(self, buf: &mut ArgumentBuffer<'q>) -> IsNull {
        if let Some(v) = self {
            v.encode(buf)
        } else {
            IsNull::Yes
        }
    }

    #[inline]
    fn encode_by_ref(&self, buf: &mut crate::sqlite::ArgumentBuffer<'q>) -> IsNull {
        if let Some(v) = self {
            v.encode_by_ref(buf)
        } else {
            IsNull::Yes
        }
    }

    #[inline]
    fn size_hint(&self) -> usize {
        self.as_ref().map_or(0, Encode::size_hint)
    }
}
