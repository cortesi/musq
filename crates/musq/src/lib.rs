mod sqlite;
mod ustr;

pub use musq_macros::*;

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

pub use either::Either;
pub use indexmap::IndexMap;

pub use crate::{
    column::Column,
    error::{DecodeError, Error, Result},
    executor::{Execute, Executor},
    from_row::FromRow,
    musq::{AutoVacuum, JournalMode, LockingMode, Musq, Synchronous},
    pool::Pool,
    query::{query, query_with},
    query_as::{query_as, query_as_with},
    query_result::QueryResult,
    query_scalar::{query_scalar, query_scalar_with},
    row::Row,
    sqlite::{
        ArgumentValue, Arguments, Connection, IntoArguments, SqliteDataType, SqliteError,
        Statement, Value,
        error::{ExtendedErrCode, PrimaryErrCode},
    },
    transaction::Transaction,
};
