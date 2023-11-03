#![cfg_attr(docsrs, feature(doc_cfg))]

pub use sqlx_core::{
    acquire::Acquire,
    arguments::{Arguments, IntoArguments},
    column::{Column, ColumnIndex},
    connection::{ConnectOptions, Connection},
    database::{self, Database},
    describe::Describe,
    executor::{Execute, Executor},
    from_row::FromRow,
    pool::{self, Pool},
    query::{query, query_with},
    query_as::{query_as, query_as_with},
    query_builder::{self, QueryBuilder},
    query_scalar::{query_scalar, query_scalar_with},
    row::Row,
    statement::Statement,
    transaction::{Transaction, TransactionManager},
    type_info::TypeInfo,
    types::Type,
    value::{Value, ValueRef},
    Either,
};

#[doc(inline)]
pub use sqlx_core::error::{self, Error, Result};

pub use sqlx_sqlite::{self as sqlite, Sqlite, SqliteConnection, SqliteExecutor, SqlitePool};
#[doc(hidden)]
pub extern crate sqlx_macros;

// derives
#[doc(hidden)]
pub use sqlx_macros::{FromRow, Type};

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
