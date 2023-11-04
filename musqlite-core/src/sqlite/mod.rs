pub use arguments::{ArgumentBuffer, ArgumentValue, Arguments};
pub use column::Column;
pub use connection::{Connection, LockedSqliteHandle};
pub use error::SqliteError;
pub use options::{
    ConnectOptions, SqliteAutoVacuum, SqliteJournalMode, SqliteLockingMode, SqliteSynchronous,
};
pub use query_result::QueryResult;
pub use row::Row;
pub use statement::Statement;
pub use transaction::TransactionManager;
pub use type_info::TypeInfo;
pub use value::{Value, ValueRef};

mod arguments;
mod column;
mod connection;
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

// NOTE: required due to the lack of lazy normalization
crate::impl_into_arguments_for_arguments!(Arguments<'q>);
crate::impl_column_index_for_row!(Row);
crate::impl_column_index_for_statement!(Statement);
crate::impl_acquire!(Sqlite, Connection);

// required because some databases have a different handling of NULL
crate::impl_encode_for_option!(Sqlite);
