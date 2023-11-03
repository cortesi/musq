//! Traits to represent a database driver.

use std::fmt::Debug;

/// A database driver.
///
/// This trait encapsulates a complete set of traits that implement a driver for a
/// specific database.
pub trait Database: 'static + Sized + Send + Debug {
    /// The concrete `QueryResult` implementation for this database.
    type QueryResult: 'static + Sized + Send + Sync + Default + Extend<Self::QueryResult>;
}

/// A [`Database`] that maintains a client-side cache of prepared statements.
pub trait HasStatementCache {}
