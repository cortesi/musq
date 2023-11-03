#![cfg_attr(docsrs, feature(doc_cfg))]

pub use sqlx_core::acquire::Acquire;
pub use sqlx_core::arguments::{Arguments, IntoArguments};
pub use sqlx_core::column::Column;
pub use sqlx_core::column::ColumnIndex;
pub use sqlx_core::connection::{ConnectOptions, Connection};
pub use sqlx_core::database::{self, Database};
pub use sqlx_core::describe::Describe;
pub use sqlx_core::executor::{Execute, Executor};
pub use sqlx_core::from_row::FromRow;
pub use sqlx_core::pool::{self, Pool};
pub use sqlx_core::query::{query, query_with};
pub use sqlx_core::query_as::{query_as, query_as_with};
pub use sqlx_core::query_builder::{self, QueryBuilder};
pub use sqlx_core::query_scalar::{query_scalar, query_scalar_with};
pub use sqlx_core::row::Row;
pub use sqlx_core::statement::Statement;
pub use sqlx_core::transaction::{Transaction, TransactionManager};
pub use sqlx_core::type_info::TypeInfo;
pub use sqlx_core::types::Type;
pub use sqlx_core::value::{Value, ValueRef};
pub use sqlx_core::Either;

#[doc(inline)]
pub use sqlx_core::error::{self, Error, Result};

pub use sqlx_sqlite::{self as sqlite, Sqlite, SqliteConnection, SqliteExecutor, SqlitePool};
#[doc(hidden)]
pub extern crate sqlx_macros;

// derives
#[doc(hidden)]
pub use sqlx_macros::{FromRow, Type};

pub use sqlx_macros::test;

#[doc(hidden)]
pub use sqlx_core::rt::test_block_on;

mod macros;

// macro support
#[doc(hidden)]
pub mod ty_match;

#[doc(hidden)]
pub use sqlx_core::rt as __rt;

/// Conversions between Rust and SQL types.
///
/// To see how each SQL type maps to a Rust type, see the corresponding `types` module for each
/// database:
///
///  * SQLite: [sqlite::types]
///
/// Any external types that have had [`Type`] implemented for, are re-exported in this module
/// for convenience as downstream users need to use a compatible version of the external crate
/// to take advantage of the implementation.
///
/// [`Type`]: types::Type
pub mod types {
    pub use sqlx_core::types::*;

    #[doc(hidden)]
    pub use sqlx_macros::Type;
}

/// Provides [`Encode`](encode::Encode) for encoding values for the database.
pub mod encode {
    pub use sqlx_core::encode::{Encode, IsNull};

    #[doc(hidden)]
    pub use sqlx_macros::Encode;
}

pub use self::encode::Encode;

/// Provides [`Decode`](decode::Decode) for decoding values from the database.
pub mod decode {
    pub use sqlx_core::decode::Decode;

    #[doc(hidden)]
    pub use sqlx_macros::Decode;
}

pub use self::decode::Decode;

/// Types and traits for the `query` family of functions and macros.
pub mod query {
    pub use sqlx_core::query::{Map, Query};
    pub use sqlx_core::query_as::QueryAs;
    pub use sqlx_core::query_scalar::QueryScalar;
}

/// Convenience re-export of common traits.
pub mod prelude {
    pub use super::Acquire;
    pub use super::ConnectOptions;
    pub use super::Connection;
    pub use super::Decode;
    pub use super::Encode;
    pub use super::Executor;
    pub use super::FromRow;
    pub use super::IntoArguments;
    pub use super::Row;
    pub use super::Statement;
    pub use super::Type;
}
