use std::sync::atomic::AtomicBool;

pub use arguments::{ArgumentBuffer, ArgumentValue, Arguments};
pub use column::SqliteColumn;
pub use connection::{LockedSqliteHandle, SqliteConnection};
pub use database::Sqlite;
pub use error::SqliteError;
pub use options::{
    SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode, SqliteLockingMode, SqliteSynchronous,
};
pub use query_result::SqliteQueryResult;
pub use row::SqliteRow;
pub use statement::SqliteStatement;
pub use transaction::SqliteTransactionManager;
pub use type_info::TypeInfo;
pub use value::{Value, ValueRef};

use crate::sqlite::connection::establish::EstablishParams;

use crate::describe::Describe;
use crate::error::Error;
use crate::executor::Executor;

mod arguments;
mod column;
mod connection;
mod database;
mod error;
mod logger;
mod options;
mod query_result;
mod row;
mod statement;
mod transaction;
mod type_info;
pub mod types;
mod value;

mod regexp;

/// An alias for [`Pool`][crate::sqlite::pool::Pool], specialized for SQLite.
pub type SqlitePool = crate::pool::Pool<Sqlite>;

/// An alias for [`PoolOptions`][crate::sqlite::pool::PoolOptions], specialized for SQLite.
pub type SqlitePoolOptions = crate::pool::PoolOptions<Sqlite>;

/// An alias for [`Executor<'_, Database = Sqlite>`][Executor].
pub trait SqliteExecutor<'c>: Executor<'c, Database = Sqlite> {}
impl<'c, T: Executor<'c, Database = Sqlite>> SqliteExecutor<'c> for T {}

// NOTE: required due to the lack of lazy normalization
crate::impl_into_arguments_for_arguments!(Arguments<'q>);
crate::impl_column_index_for_row!(SqliteRow);
crate::impl_column_index_for_statement!(SqliteStatement);
crate::impl_acquire!(Sqlite, SqliteConnection);

// required because some databases have a different handling of NULL
crate::impl_encode_for_option!(Sqlite);

/// UNSTABLE: for use by `sqlx-cli` only.
#[doc(hidden)]
pub static CREATE_DB_WAL: AtomicBool = AtomicBool::new(true);

/// UNSTABLE: for use by `sqlite-macros-core` only.
#[doc(hidden)]
pub fn describe_blocking(query: &str, database_url: &str) -> Result<Describe<Sqlite>, Error> {
    let opts: SqliteConnectOptions = database_url.parse()?;
    let params = EstablishParams::from_options(&opts)?;
    let mut conn = params.establish()?;

    // Execute any ancillary `PRAGMA`s
    connection::execute::iter(&mut conn, &opts.pragma_string(), None, false)?.finish()?;

    connection::describe::describe(&mut conn, query)

    // SQLite database is closed immediately when `conn` is dropped
}
