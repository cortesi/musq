//! An async SQLite driver focused on performance, correctness, and flexibility.

/// SQLite backend implementation.
mod sqlite;
#[macro_use]
/// Macro helper for enum mode definitions.
mod enum_mode;

pub use musq_macros::*;

/// Column metadata utilities.
mod column;
/// Debug formatting helpers.
mod debugfn;
/// Decoding support for database values.
pub mod decode;
/// Encoding support for database values.
pub mod encode;
/// Error types and result helpers.
pub mod error;
/// Query execution trait and adapters.
mod executor;
/// Row decoding support.
mod from_row;
#[macro_use]
/// Logging utilities.
mod logger;
/// SQL expression helpers for dynamic queries.
pub mod expr;
/// Connection options and configuration.
mod musq;
/// Connection pool implementation.
mod pool;
/// Query types and helpers.
pub mod query;
/// Query builder helpers.
mod query_builder;
/// Query execution results.
mod query_result;
/// Row representation.
mod row;
/// Prepared statement cache.
mod statement_cache;
/// Transaction handling.
mod transaction;
/// Value collection helpers.
mod values;
/// Built-in type adapters.
#[macro_use]
pub mod types;

pub use crate::{
    encode::Null,
    error::{DecodeError, EncodeError, Error, Result},
    executor::Execute,
    expr::Expr,
    from_row::{AllNull, FromRow},
    musq::{AutoVacuum, JournalMode, LockingMode, Musq, Synchronous},
    pool::{Pool, PoolConnection},
    query::{
        Query, query, query_as, query_as_with, query_scalar, query_scalar_with, query_with,
        quote_identifier,
    },
    query_builder::QueryBuilder,
    query_result::QueryResult,
    row::Row,
    sqlite::{Arguments, Connection, Prepared, SqliteDataType, SqliteError, Value},
    transaction::Transaction,
    values::{IntoValuesEntry, Values, ValuesEntry},
};

#[macro_export]
/// Build a [`Values`](crate::Values) collection from literal key/value pairs.
macro_rules! values {
    () => { $crate::Result::<$crate::Values>::Ok($crate::Values::new()) };
    { $($key:literal : $value:expr),* $(,)? } => {{
        let mut _values = $crate::Values::new();
        $( _values.insert($key, $value)?; )*
        $crate::Result::<$crate::Values>::Ok(_values)
    }};
}
