#![cfg_attr(docsrs, feature(doc_cfg))]

//! An async SQLite driver focused on performance, correctness, and flexibility.
//!
//! Musq supports the SQLite release bundled by the pinned `libsqlite3-sys`
//! crate. It does not support linking against older or system SQLite libraries.
//! Read the bundled version from [`BUNDLED_SQLITE_VERSION`] or
//! [`SqliteRuntimeInfo`].

/// SQLite backend implementation.
mod sqlite;
#[macro_use]
/// Macro helper for enum mode definitions.
mod enum_mode;

pub use musq_macros::*;

/// SQLite release bundled by the pinned `libsqlite3-sys` crate.
///
/// Compare this value with [`SqliteRuntimeInfo::version`] from
/// `runtime_info()`.
pub const BUNDLED_SQLITE_VERSION: &str = env!("BUNDLED_SQLITE_VERSION");

/// Re-exports used by proc-macro expansions.
#[cfg(feature = "json")]
#[doc(hidden)]
pub mod __private {
    pub use serde_json;
}

/// Typed condition collection.
mod conditions;
/// High-level database host, module registry, and read-only sandbox.
mod database;
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
/// SQL expression helpers for dynamic queries.
pub mod expr;
/// Row decoding support.
mod from_row;
/// Logging utilities.
mod logger;
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

#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub use crate::database::{json_params_to_values, value_to_json};
#[cfg(feature = "vec")]
#[cfg_attr(docsrs, doc(cfg(feature = "vec")))]
pub use crate::types::vec::{VecBit, VecF32, VecInt8};
pub use crate::{
    conditions::Conditions,
    database::{
        Database, DatabaseOptions, Module, ModuleRecord, QuerySurface, ReadOnlySandbox,
        SandboxPrepareHook, SandboxQueryOptions, SandboxQueryResult, fleet_query,
        validate_single_statement,
    },
    encode::Null,
    error::{DecodeError, EncodeError, Error, ModuleError, Result, SandboxError},
    executor::Execute,
    expr::Expr,
    from_row::{AllNull, FromRow},
    musq::{AutoVacuum, JournalMode, LockingMode, Musq, Synchronous},
    pool::{Pool, PoolConnection, PoolStats},
    query::{
        Query, QueryExecutor, query, query_as, query_as_with, query_scalar, query_scalar_with,
        query_with, quote_identifier,
    },
    query_builder::QueryBuilder,
    query_result::QueryResult,
    row::Row,
    sqlite::{
        ActionCode, Arguments, AuthResult, BackupReport, Connection, DbStatus, DbStatusKind,
        DeserializeMode, ForeignKeyViolation, FunctionFlags, IntegrityReport, InterruptHandle,
        SqliteDataType, SqliteError, SqliteLimit, SqliteRuntimeInfo, Text, UpdateEvent, UpdateOp,
        Value, WalCheckpoint, WalCheckpointMode,
    },
    transaction::{Transaction, TransactionBehavior, TxnState},
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
