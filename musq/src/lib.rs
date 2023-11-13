mod sqlite;
mod ustr;

pub use musq_macros::*;

#[macro_use]
pub mod async_stream;

mod acquire;
mod column;
mod debugfn;
pub mod decode;
pub mod encode;
mod error;
mod executor;
mod from_row;
mod logger;
mod musq;
pub mod pool;
pub mod query;
mod query_as;
mod query_result;
mod query_scalar;
mod row;
mod statement_cache;
mod transaction;
pub mod types;

/// sqlx uses ahash for increased performance, at the cost of reduced DoS resistance.
use ahash::AHashMap as HashMap;
pub use either::Either;
pub use indexmap::IndexMap;

pub use crate::{
    column::Column,
    error::{Error, Result},
    executor::{Execute, Executor},
    from_row::FromRow,
    musq::{AutoVacuum, JournalMode, LockingMode, Musq, Synchronous},
    pool::{Pool, PoolOptions},
    query::{query, query_with},
    query_as::{query_as, query_as_with},
    query_result::QueryResult,
    query_scalar::{query_scalar, query_scalar_with},
    row::Row,
    sqlite::{
        error::{ExtendedErrCode, PrimaryErrCode},
        ArgumentBuffer, Arguments, Connection, IntoArguments, SqliteDataType, SqliteError,
        Statement, Value,
    },
    transaction::Transaction,
    types::Type,
};
