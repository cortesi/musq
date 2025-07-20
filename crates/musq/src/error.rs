//! Types for working with errors produced by Musq.

use std::io;
use std::num::TryFromIntError;
use std::sync::PoisonError;

use tokio::sync::TryLockError;

use crate::{SqliteDataType, sqlite, sqlite::error::SqliteError};

/// A specialized `Result` type for Musq.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error("incompatible source data type: {0}")]
    DataType(SqliteDataType),
    #[error("decoding conversion error: {0}")]
    Conversion(String),
}

impl From<TryFromIntError> for DecodeError {
    fn from(err: TryFromIntError) -> Self {
        DecodeError::Conversion(err.to_string())
    }
}

impl From<String> for DecodeError {
    fn from(err: String) -> Self {
        DecodeError::Conversion(err)
    }
}

/// Represents all the ways a method can fail within Musq.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Error returned from the database.
    #[error("error returned from database: {0}")]
    Sqlite(#[source] sqlite::error::SqliteError),

    /// Error communicating with the database backend.
    #[error("error communicating with database: {0}")]
    Io(#[from] io::Error),

    /// Unexpected or invalid data encountered while communicating with the database.
    ///
    /// This should indicate there is a programming error in Musq or there
    /// is something corrupted with the connection to the database itself.
    #[error("encountered unexpected or invalid data: {0}")]
    Protocol(String),

    /// No rows returned by a query that expected to return at least one row.
    #[error("no rows returned by a query that expected to return at least one row")]
    RowNotFound,

    /// Type in query doesn't exist. Likely due to typo or missing user type.
    #[error("type named {type_name} not found")]
    TypeNotFound { type_name: String },

    /// Column index was out of bounds.
    #[error("column index out of bounds: the len is {len}, but the index is {index}")]
    ColumnIndexOutOfBounds { index: usize, len: usize },

    /// No column found for the given name.
    #[error("no column found for name: {0}")]
    ColumnNotFound(String),

    /// Error occurred while decoding a value from a specific column.
    #[error("error occurred while decoding column {index}: {source}")]
    ColumnDecode {
        index: String,

        #[source]
        source: DecodeError,
    },

    /// Error occurred while decoding a value.
    #[error("error occurred while decoding: {0}")]
    Decode(#[source] DecodeError),

    /// A [`Pool::acquire`] timed out due to connections not becoming available or
    /// because another task encountered too many errors while trying to open a new connection.
    ///
    /// [`Pool::acquire`]: crate::pool::Pool::acquire
    #[error("pool timed out while waiting for an open connection")]
    PoolTimedOut,

    /// [`Pool::close`] was called while we were waiting in [`Pool::acquire`].
    ///
    /// [`Pool::acquire`]: crate::pool::Pool::acquire
    /// [`Pool::close`]: crate::pool::Pool::close
    #[error("attempted to acquire a connection on a closed pool")]
    PoolClosed,

    /// A background worker has crashed.
    #[error("attempted to communicate with a crashed background worker")]
    WorkerCrashed,

    /// [`sqlite3_unlock_notify`] kept returning `SQLITE_LOCKED` even after
    /// resetting the blocking statement.
    #[error("unlock_notify failed after multiple attempts")]
    UnlockNotify,
}

impl Error {
    pub fn into_sqlite_error(self) -> Option<sqlite::error::SqliteError> {
        match self {
            Error::Sqlite(err) => Some(err),
            _ => None,
        }
    }
}

impl From<SqliteError> for Error {
    fn from(error: SqliteError) -> Self {
        Error::Sqlite(error)
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(_: PoisonError<T>) -> Self {
        Error::WorkerCrashed
    }
}

impl From<TryLockError> for Error {
    fn from(_: TryLockError) -> Self {
        Error::WorkerCrashed
    }
}
