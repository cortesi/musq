//! Traits to represent a database driver.
//!
//! ## Example
//!
//! ```rust,ignore
//! // connect to SQLite
//! let conn = AnyConnection::connect("sqlite://file.db").await?;

use std::fmt::Debug;

use crate::arguments::Arguments;
use crate::column::Column;
use crate::connection::Connection;
use crate::row::Row;

use crate::statement::Statement;
use crate::transaction::TransactionManager;

/// A database driver.
///
/// This trait encapsulates a complete set of traits that implement a driver for a
/// specific database.
pub trait Database:
    'static
    + Sized
    + Send
    + Debug
    + for<'q> HasArguments<'q, Database = Self>
    + for<'q> HasStatement<'q, Database = Self>
{
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

/// Associate [`Database`] with an [`Arguments`](crate::arguments::Arguments) of a generic lifetime.
///
/// ---
///
/// The upcoming Rust feature, [Generic Associated Types], should obviate
/// the need for this trait.
///
/// [Generic Associated Types]: https://github.com/rust-lang/rust/issues/44265
pub trait HasArguments<'q> {
    type Database: Database;

    /// The concrete `Arguments` implementation for this database.
    type Arguments: Arguments<'q, Database = Self::Database>;

    /// The concrete type used as a buffer for arguments while encoding.
    type ArgumentBuffer;
}

/// Associate [`Database`] with a [`Statement`](crate::statement::Statement) of a generic lifetime.
///
/// ---
///
/// The upcoming Rust feature, [Generic Associated Types], should obviate
/// the need for this trait.
///
/// [Generic Associated Types]: https://github.com/rust-lang/rust/issues/44265
pub trait HasStatement<'q> {
    type Database: Database;

    /// The concrete `Statement` implementation for this database.
    type Statement: Statement<'q, Database = Self::Database>;
}

/// A [`Database`] that maintains a client-side cache of prepared statements.
pub trait HasStatementCache {}
