//! Conversions between Rust and **SQLite** types.
//!
//! # Types
//!
//! | Rust type                             | SQLite type(s)      |
//! |---------------------------------------|---------------------|
//! | `bool`                                | BOOLEAN             |
//! | `i8`                                  | INTEGER             |
//! | `i16`                                 | INTEGER             |
//! | `i32`                                 | INTEGER             |
//! | `i64`                                 | BIGINT, INT8        |
//! | `u8`                                  | INTEGER             |
//! | `u16`                                 | INTEGER             |
//! | `u32`                                 | INTEGER             |
//! | `u64`                                 | INTEGER             |
//! | `usize`                               | INTEGER             |
//! | `f32`                                 | REAL                |
//! | `f64`                                 | REAL                |
//! | `&str`, [`String`]                    | TEXT                |
//! | `&[u8]`, `Vec<u8>`                    | BLOB                |
//! | `VecF32`*                             | BLOB                |
//! | `VecInt8`*                            | BLOB                |
//! | `VecBit`*                             | BLOB                |
//! | `time::PrimitiveDateTime`             | DATETIME            |
//! | `time::OffsetDateTime`                | DATETIME            |
//! | `time::Date`                          | DATE                |
//! | `time::Time`                          | TIME                |
//! | `bstr::BString`                       | BLOB                |
//! | `std::path::Path`, `PathBuf`           | TEXT                |
//! | `serde_json::Value`                    | TEXT                |
//!
//! `*` Requires the `vec` feature.
//!
//! #### Note: Unsigned Integers
//!
//! The unsigned integer types use SQLite's signed 64-bit integer storage. Encoding a `u64` or
//! `usize` value above `i64::MAX` returns an error. Decoding any negative value returns an error.
//!
//! SQLite stores integers in a variable-width encoding and always handles them in memory as 64-bit signed values, so no
//! space is wasted by this implicit widening.
//!
//! Values outside SQLite's signed integer range are not stored as `REAL`, `BLOB`, or `TEXT`.
//!
//! # Nullable
//!
//! `Option<T>` is supported where `T` implements `Encode` or `Decode`. An `Option<T>` represents a potentially `NULL`
//! value from SQLite.

/// Ensure a value is compatible with the expected SQLite type.
macro_rules! compatible {
    ($x:expr, $($y:path)|+) => {
        let t = $x.type_info();
        if !t.is_null() && !matches!(t, $($y)|+) {
            return Err(DecodeError::IncompatibleDataType(t))
        }
    };
}

/// Conversions for `bstr` text types.
pub mod bstr;
/// JSON value conversions.
mod json;
/// Filesystem path conversions.
mod path;
/// Conversions for `time` crate types.
pub mod time;
/// Vector conversions for sqlite-vec (`feature = "vec"`).
#[cfg(feature = "vec")]
#[cfg_attr(docsrs, doc(cfg(feature = "vec")))]
pub mod vec;

/// Bool type conversions.
mod bool;
/// Byte slice conversions.
mod bytes;
/// Floating-point conversions.
mod float;
/// Integer conversions.
mod int;
/// String conversions.
mod str;
/// Unsigned integer conversions.
mod uint;
