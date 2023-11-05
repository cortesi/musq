pub mod sqlite;

mod ustr;

#[macro_use]
pub mod async_stream;

#[macro_use]
pub mod error;

#[macro_use]
pub mod pool;

pub mod connection;

#[macro_use]
pub mod transaction;

#[macro_use]
pub mod encode;

#[macro_use]
pub mod decode;

#[macro_use]
pub mod types;

#[macro_use]
pub mod query;

#[macro_use]
pub mod acquire;

#[macro_use]
pub mod column;

pub mod debugfn;
pub mod describe;
pub mod executor;
pub mod from_row;
pub mod logger;
pub mod query_as;
pub mod query_builder;
mod query_result;
pub mod query_scalar;
pub mod row;
pub mod statement_cache;

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
        Arguments, ConnectOptions, Connection, IntoArguments, SqliteDataType, Statement, Value,
        ValueRef,
    },
    transaction::{Transaction, TransactionManager},
    types::Type,
};
