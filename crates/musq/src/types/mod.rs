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
//! | `f32`                                 | REAL                |
//! | `f64`                                 | REAL                |
//! | `&str`, [`String`]                    | TEXT                |
//! | `&[u8]`, `Vec<u8>`                    | BLOB                |
//! | `time::PrimitiveDateTime`             | DATETIME            |
//! | `time::OffsetDateTime`                | DATETIME            |
//! | `time::Date`                          | DATE                |
//! | `time::Time`                          | TIME                |
//! | `bstr::BString`                       | BLOB                |
//!
//! #### Note: Unsigned Integers
//!
//! The unsigned integer types `u8`, `u16` and `u32` are implemented by zero-extending to the next-larger signed type.
//! So `u8` becomes `i16`, `u16` becomes `i32`, and `u32` becomes `i64` while still retaining their semantic values.
//!
//! SQLite stores integers in a variable-width encoding and always handles them in memory as 64-bit signed values, so no
//! space is wasted by this implicit widening.
//!
//! There is no corresponding larger type for `u64` in SQLite (it would require a `i128`), and so it is not supported.
//! Bit-casting it to `i64` or storing it as `REAL`, `BLOB` or `TEXT` would change the semantics of the value in SQL and
//! so violates the principle of least surprise.
//!
//! # Nullable
//!
//! `Option<T>` is supported where `T` implements `Encode` or `Decode`. An `Option<T>` represents a potentially `NULL`
//! value from SQLite.

macro_rules! compatible {
    ($x:expr, $($y:path)|+) => {
        let t = $x.type_info();
        if !t.is_null() && !matches!(t, $($y)|+) {
            return Err(DecodeError::DataType(t))
        }
    };
}

pub mod bstr;
pub mod time;

mod bool;
mod bytes;
mod float;
mod int;
mod str;
mod uint;
