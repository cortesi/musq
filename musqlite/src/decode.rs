//! Provides [`Decode`] for decoding values from the database.

use crate::{error::BoxDynError, ValueRef};

/// A type that can be decoded from the database.
///
/// ## How can I implement `Decode`?
///
/// A manual implementation of `Decode` can be useful when adding support for
/// types externally to SQLx.
///
/// ```rust
/// # use musqlite::decode::Decode;
/// # use musqlite::types::Type;
/// # use musqlite::{SqliteDataType, ValueRef};
/// # use std::error::Error;
/// #
/// struct MyType;
///
/// # impl Type for MyType {
/// # fn type_info() -> SqliteDataType { todo!() }
/// # }
/// #
/// # impl std::str::FromStr for MyType {
/// # type Err = musqlite::Error;
/// # fn from_str(s: &str) -> Result<Self, Self::Err> { todo!() }
/// # }
/// #
/// // DB is the database driver
/// // `'r` is the lifetime of the `Row` being decoded
/// impl<'r> Decode<'r> for MyType
/// where
///     // we want to delegate some of the work to string decoding so let's make sure strings
///     // are supported by the database
///     &'r str: Decode<'r>
/// {
///     fn decode(
///         value: ValueRef<'r>,
///     ) -> Result<MyType, Box<dyn Error + 'static + Send + Sync>> {
///         // the interface of ValueRef is largely unstable at the moment
///         // so this is not directly implementable
///
///         // however, you can delegate to a type that matches the format of the type you want
///         // to decode (such as a UTF-8 string)
///
///         let value = <&str as Decode>::decode(value)?;
///
///         // now you can parse this into your type (assuming there is a `FromStr`)
///
///         Ok(value.parse()?)
///     }
/// }
/// ```
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
