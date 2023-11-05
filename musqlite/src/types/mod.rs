//! Conversions between Rust and **SQLite** types.
//!
//! # Types
//!
//! | Rust type                             | SQLite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | `bool`                                | BOOLEAN                                              |
//! | `i8`                                  | INTEGER                                              |
//! | `i16`                                 | INTEGER                                              |
//! | `i32`                                 | INTEGER                                              |
//! | `i64`                                 | BIGINT, INT8                                         |
//! | `u8`                                  | INTEGER                                              |
//! | `u16`                                 | INTEGER                                              |
//! | `u32`                                 | INTEGER                                              |
//! | `f32`                                 | REAL                                                 |
//! | `f64`                                 | REAL                                                 |
//! | `&str`, [`String`]                    | TEXT                                                 |
//! | `&[u8]`, `Vec<u8>`                    | BLOB                                                 |
//!
//! #### Note: Unsigned Integers
//! The unsigned integer types `u8`, `u16` and `u32` are implemented by zero-extending to the
//! next-larger signed type. So `u8` becomes `i16`, `u16` becomes `i32`, and `u32` becomes `i64`
//! while still retaining their semantic values.
//!
//! Similarly, decoding performs a checked truncation to ensure that overflow does not occur.
//!
//! SQLite stores integers in a variable-width encoding and always handles them in memory as 64-bit
//! signed values, so no space is wasted by this implicit widening.
//!
//! However, there is no corresponding larger type for `u64` in SQLite (it would require a `i128`),
//! and so it is not supported. Bit-casting it to `i64` or storing it as `REAL`, `BLOB` or `TEXT`
//! would change the semantics of the value in SQL and so violates the principle of least surprise.
//!
//! ### [`time`](https://crates.io/crates/time)
//!
//! | Rust type                             | Sqlite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | `time::PrimitiveDateTime`             | DATETIME                                             |
//! | `time::OffsetDateTime`                | DATETIME                                             |
//! | `time::Date`                          | DATE                                                 |
//! | `time::Time`                          | TIME                                                 |
//!
//! ### [`json`](https://crates.io/crates/serde_json)
//!
//! | Rust type                             | Sqlite type(s)                                       |
//! |---------------------------------------|------------------------------------------------------|
//! | [`Json<T>`]                           | TEXT                                                 |
//! | `serde_json::JsonValue`               | TEXT                                                 |
//! | `&serde_json::value::RawValue`        | TEXT                                                 |
//!
//! # Nullable
//!
//! In addition, `Option<T>` is supported where `T` implements `Type`. An `Option<T>` represents
//! a potentially `NULL` value from SQLite.

use crate::sqlite;

mod bool;
mod bytes;
mod float;
mod int;
mod str;
pub mod time;
mod uint;

pub mod bstr;
mod json;

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
    fn type_info() -> sqlite::SqliteDataType;

    /// Determines if this Rust type is compatible with the given SQL type.
    ///
    /// When decoding values from a row, this method is checked to determine if we should continue
    /// or raise a runtime type mismatch error.
    ///
    /// When binding arguments with `query!` or `query_as!`, this method is consulted to determine
    /// if the Rust type is acceptable.
    fn compatible(ty: &sqlite::SqliteDataType) -> bool {
        *ty == Self::type_info()
    }
}

// for references, the underlying SQL type is identical
impl<T: ?Sized + Type> Type for &'_ T {
    fn type_info() -> sqlite::SqliteDataType {
        <T as Type>::type_info()
    }

    fn compatible(ty: &sqlite::SqliteDataType) -> bool {
        <T as Type>::compatible(ty)
    }
}

// for optionals, the underlying SQL type is identical
impl<T: Type> Type for Option<T> {
    fn type_info() -> sqlite::SqliteDataType {
        <T as Type>::type_info()
    }

    fn compatible(ty: &sqlite::SqliteDataType) -> bool {
        <T as Type>::compatible(ty)
    }
}
