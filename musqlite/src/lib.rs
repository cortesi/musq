mod sqlite;
mod ustr;

pub use musqlite_macros::*;

#[macro_use]
pub mod async_stream;

mod acquire;
mod column;
mod connection;
mod debugfn;
pub mod decode;
pub mod encode;
mod error;
mod executor;
mod from_row;
mod logger;
pub mod pool;
pub mod query;
mod query_as;
mod query_builder;
mod query_result;
mod query_scalar;
mod row;
mod statement_cache;
mod transaction;
pub mod types;

/// sqlx uses ahash for increased performance, at the cost of reduced DoS resistance.
pub use ahash::AHashMap as HashMap;
pub use either::Either;
pub use indexmap::IndexMap;

pub use crate::{
    column::{Column, ColumnIndex},
    error::{Error, Result},
    executor::{Execute, Executor},
    from_row::FromRow,
    pool::{Pool, PoolOptions},
    query::{query, query_with},
    query_as::{query_as, query_as_with},
    query_builder::QueryBuilder,
    query_result::QueryResult,
    query_scalar::{query_scalar, query_scalar_with},
    row::Row,
    sqlite::{
        error::{ExtendedErrCode, PrimaryErrCode},
        ArgumentBuffer, Arguments, AutoVacuum, ConnectOptions, Connection, IntoArguments,
        JournalMode, LockingMode, SqliteDataType, SqliteError, Statement, Synchronous, Value,
        ValueRef,
    },
    transaction::{Transaction, TransactionManager},
    types::Type,
};
