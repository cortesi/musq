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
use crate::value::{Value, ValueRef};

/// A database driver.
///
/// This trait encapsulates a complete set of traits that implement a driver for a
/// specific database.
pub trait Database:
    'static
    + Sized
    + Send
    + Debug
    + for<'r> HasValueRef<'r, Database = Self>
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

    /// The concrete type used to hold an owned copy of the not-yet-decoded value that was
    /// received from the database.
    type Value: Value<Database = Self> + 'static;
}

/// Associate [`Database`] with a [`ValueRef`](crate::value::ValueRef) of a generic lifetime.
///
/// ---
///
/// The upcoming Rust feature, [Generic Associated Types], should obviate
/// the need for this trait.
///
/// [Generic Associated Types]: https://github.com/rust-lang/rust/issues/44265
pub trait HasValueRef<'r> {
    type Database: Database;

    /// The concrete type used to hold a reference to the not-yet-decoded value that has just been
    /// received from the database.
    type ValueRef: ValueRef<'r, Database = Self::Database>;
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
