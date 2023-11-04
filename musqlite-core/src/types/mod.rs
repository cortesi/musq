//! Conversions between Rust and SQLite types.
//!
//! # Nullable
//!
//! To represent nullable SQL types, `Option<T>` is supported where `T` implements `Type`.
//! An `Option<T>` represents a potentially `NULL` value from SQL.
//!

use crate::sqlite;

pub mod bstr;
mod json;
pub use uuid::{self, Uuid};

pub use sqlite::types::chrono;
pub use sqlite::types::time;

#[doc(no_inline)]
pub use bit_vec::BitVec;
pub use json::{Json, JsonRawValue, JsonValue};

/// Indicates that a SQL type is supported.
///
/// ## Derivable
///
/// This trait can be derived by SQLx to support Rust-only wrapper types, enumerations, and structured records.
/// Additionally, an implementation of [`Encode`](crate::encode::Encode) and [`Decode`](crate::decode::Decode) is
/// generated.
///
/// ### Transparent
///
/// Rust-only domain wrappers around SQL types. The generated implementations directly delegate to the implementation of
/// the inner type.
///
/// ```rust,ignore
/// #[derive(Type)]
/// #[sqlx(transparent)]
/// struct UserId(i64);
/// ```
///
/// ##### Attributes
///
/// * `#[sqlx(rename_all = "<strategy>")]` on struct definition: See [`derive docs in
///   FromRow`](crate::from_row::FromRow#rename_all)
/// * `#[sqlx(no_pg_array)]`: do not emit a `PgHasArrayType` impl (see above).
///
/// ### Enumeration
///
/// Enumerations may be defined in Rust and can match SQL by integer discriminant or variant name.
///
/// With `#[repr(_)]` the integer representation is used when converting from/to SQL and expects that SQL type (e.g.,
/// `INT`). Without, the names of the variants are used instead and expects a textual SQL type (e.g., `VARCHAR`,
/// `TEXT`).
///
/// ```rust,ignore
/// #[derive(Type)]
/// #[repr(i32)]
/// enum Color { Red = 1, Green = 2, Blue = 3 }
/// ```
///
/// ```rust,ignore
/// #[derive(Type)]
/// #[sqlx(rename_all = "lowercase")]
/// enum Color { Red, Green, Blue }
/// ```
///
pub trait Type {
    /// Returns the canonical SQL type for this Rust type.
    ///
    /// When binding arguments, this is used to tell the database what is about to be sent; which,
    /// the database then uses to guide query plans. This can be overridden by `Encode::produces`.
    ///
    /// A map of SQL types to Rust types is populated with this and used
    /// to determine the type that is returned from the anonymous struct type from `query!`.
    fn type_info() -> sqlite::TypeInfo;

    /// Determines if this Rust type is compatible with the given SQL type.
    ///
    /// When decoding values from a row, this method is checked to determine if we should continue
    /// or raise a runtime type mismatch error.
    ///
    /// When binding arguments with `query!` or `query_as!`, this method is consulted to determine
    /// if the Rust type is acceptable.
    fn compatible(ty: &sqlite::TypeInfo) -> bool {
        *ty == Self::type_info()
    }
}

// for references, the underlying SQL type is identical
impl<T: ?Sized + Type> Type for &'_ T {
    fn type_info() -> sqlite::TypeInfo {
        <T as Type>::type_info()
    }

    fn compatible(ty: &sqlite::TypeInfo) -> bool {
        <T as Type>::compatible(ty)
    }
}

// for optionals, the underlying SQL type is identical
impl<T: Type> Type for Option<T> {
    fn type_info() -> sqlite::TypeInfo {
        <T as Type>::type_info()
    }

    fn compatible(ty: &sqlite::TypeInfo) -> bool {
        <T as Type>::compatible(ty)
    }
}
