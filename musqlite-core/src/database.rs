//! Traits to represent a database driver.
//!
//! ## Example
//!
//! ```rust,ignore
//! // connect to SQLite
//! let conn = AnyConnection::connect("sqlite://file.db").await?;

use std::fmt::Debug;

use crate::column::Column;
use crate::connection::Connection;
use crate::row::Row;

use crate::transaction::TransactionManager;

/// A database driver.
///
/// This trait encapsulates a complete set of traits that implement a driver for a
/// specific database.
pub trait Database: 'static + Sized + Send + Debug {
    /// The concrete `Connection` implementation for this database.
    type Connection: Connection<Database = Self>;

    /// The concrete `TransactionManager` implementation for this database.
    type TransactionManager: TransactionManager<Database = Self>;

    /// The concrete `Row` implementation for this database.
    type Row: Row<Database = Self>;

    /// The concrete `QueryResult` implementation for this database.
    type QueryResult: 'static + Sized + Send + Sync + Default + Extend<Self::QueryResult>;

    /// The concrete `Column` implementation for this database.
    type Column: Column<Database = Self>;
}

/// A [`Database`] that maintains a client-side cache of prepared statements.
pub trait HasStatementCache {}
