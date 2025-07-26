mod sqlite;
#[macro_use]
mod enum_mode;

pub use musq_macros::*;

mod column;
mod debugfn;
pub mod decode;
pub mod encode;
pub mod error;
mod executor;
mod from_row;
#[macro_use]
mod logger;
mod musq;
mod pool;
pub mod query;
mod query_builder;
mod query_result;
mod row;
mod statement_cache;
mod transaction;
mod values;
#[macro_use]
pub mod types;

pub use crate::{
    error::{DecodeError, EncodeError, Error, Result},
    executor::Execute,
    from_row::{AllNull, FromRow},
    musq::{AutoVacuum, JournalMode, LockingMode, Musq, Synchronous},
    pool::{Pool, PoolConnection},
    query::quote_identifier,
    query::{Query, query, query_as, query_as_with, query_scalar, query_scalar_with, query_with},
    query_builder::QueryBuilder,
    query_result::QueryResult,
    row::Row,
    sqlite::{Arguments, Connection, Prepared, SqliteDataType, SqliteError, Value},
    transaction::Transaction,
    values::Values,
};

#[macro_export]
macro_rules! values {
    () => { ::musq::Result::<$crate::Values>::Ok($crate::Values::new()) };
    { $($key:literal : $value:expr),* $(,)? } => {{
        let mut _values = $crate::Values::new();
        $( _values.insert($key, $value)?; )*
        ::musq::Result::<$crate::Values>::Ok(_values)
    }};
}
