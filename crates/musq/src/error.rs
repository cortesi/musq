//! Types for working with errors produced by Musq.

use std::{io, num::TryFromIntError, result::Result as StdResult, sync::PoisonError};

use tokio::sync::TryLockError;

pub use crate::sqlite::error::{ExtendedErrCode, PrimaryErrCode};
use crate::{
    SqliteDataType, sqlite,
    sqlite::{Value, error::SqliteError},
};

/// A specialized `Result` type for Musq.
pub type Result<T> = StdResult<T, Error>;

/// Errors encountered while decoding values.
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    /// Incompatible source SQLite type.
    #[error("incompatible source data type: {0}")]
    IncompatibleDataType(SqliteDataType),
    /// Conversion error from SQLite value to Rust type.
    #[error("decoding conversion error: {0}")]
    Conversion(String),
}

/// Errors encountered while encoding values.
#[derive(thiserror::Error, Debug)]
pub enum EncodeError {
    /// Conversion error from Rust type to SQLite value.
    #[error("encoding conversion error: {0}")]
    Conversion(String),
}

impl From<TryFromIntError> for DecodeError {
    fn from(err: TryFromIntError) -> Self {
        Self::Conversion(err.to_string())
    }
}

impl From<String> for DecodeError {
    fn from(err: String) -> Self {
        Self::Conversion(err)
    }
}

impl From<String> for EncodeError {
    fn from(err: String) -> Self {
        Self::Conversion(err)
    }
}

/// Represents all the ways a method can fail within Musq.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Error returned from the database.
    #[error(
        "error returned from database (primary: {primary:?}, extended: {extended:?}): {message}"
    )]
    Sqlite {
        /// Primary SQLite error code.
        primary: PrimaryErrCode,
        /// Extended SQLite error code.
        extended: ExtendedErrCode,
        /// SQLite-provided error message.
        message: String,
    },

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
    TypeNotFound {
        /// Name of the missing type.
        type_name: String,
    },

    /// Column index was out of bounds.
    #[error("column index out of bounds: the len is {len}, but the index is {index}")]
    ColumnIndexOutOfBounds {
        /// Out-of-range index.
        index: usize,
        /// Available column count.
        len: usize,
    },

    /// No column found for the given name.
    #[error("no column found for name: {0}")]
    ColumnNotFound(String),

    /// Encountered an unknown column type code.
    #[error("unknown column type: {0}")]
    UnknownColumnType(i32),

    /// Error occurred while decoding a value from a specific column.
    #[error(
        "error occurred while decoding column {column_name} at index {index} (value: {value:?}): {source}"
    )]
    ColumnDecode {
        /// Column index or label.
        index: String,
        /// Column name.
        column_name: String,
        /// Raw SQLite value.
        value: Value,

        #[source]
        /// Underlying decode error.
        source: DecodeError,
    },

    /// Error occurred while decoding a value.
    #[error("error occurred while decoding: {0}")]
    Decode(#[source] DecodeError),

    /// Error occurred while encoding a value.
    #[error("error occurred while encoding: {0}")]
    Encode(#[source] EncodeError),

    /// A [`Pool::acquire`] timed out due to connections not becoming available or
    /// because another task encountered too many errors while trying to open a new connection.
    ///
    /// [`Pool::acquire`]: crate::Pool::acquire
    #[error("pool timed out while waiting for an open connection")]
    PoolTimedOut,

    /// [`Pool::close`] was called while we were waiting in [`Pool::acquire`].
    ///
    /// [`Pool::acquire`]: crate::Pool::acquire
    /// [`Pool::close`]: crate::Pool::close
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
    /// Convert this error into a SQLite error if it originated there.
    pub fn into_sqlite_error(self) -> Option<sqlite::error::SqliteError> {
        match self {
            Self::Sqlite {
                primary,
                extended,
                message,
            } => Some(sqlite::error::SqliteError {
                primary,
                extended,
                message,
            }),
            _ => None,
        }
    }
}

impl From<SqliteError> for Error {
    fn from(error: SqliteError) -> Self {
        Self::Sqlite {
            primary: error.primary,
            extended: error.extended,
            message: error.message,
        }
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(_: PoisonError<T>) -> Self {
        Self::WorkerCrashed
    }
}

impl From<TryLockError> for Error {
    fn from(_: TryLockError) -> Self {
        Self::WorkerCrashed
    }
}
