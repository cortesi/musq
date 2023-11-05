mod sqlite;

mod ustr;

#[macro_use]
pub mod async_stream;
#[macro_use]
pub mod error;

pub mod acquire;
pub mod column;
pub mod connection;
pub mod debugfn;
pub mod decode;
pub mod describe;
pub mod encode;
pub mod executor;
pub mod from_row;
pub mod logger;
pub mod pool;
pub mod query;
pub mod query_as;
pub mod query_builder;
mod query_result;
pub mod query_scalar;
mod row;
pub mod statement_cache;
pub mod transaction;
pub mod types;

pub use error::{Error, Result};

/// sqlx uses ahash for increased performance, at the cost of reduced DoS resistance.
pub use ahash::AHashMap as HashMap;
pub use bytes;
pub use either::Either;
pub use indexmap::IndexMap;
pub use percent_encoding;
pub use smallvec::SmallVec;

pub use crate::{
    acquire::Acquire,
    column::{Column, ColumnIndex},
    describe::Describe,
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
        ArgumentBuffer, Arguments, ConnectOptions, Connection, IntoArguments, SqliteDataType,
        Statement, Value, ValueRef,
    },
    transaction::{Transaction, TransactionManager},
    types::Type,
};
