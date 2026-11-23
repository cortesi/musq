//! Types for working with errors produced by Musq.

use std::{
    io, num::TryFromIntError, path::PathBuf, result::Result as StdResult, sync::PoisonError,
};

pub use crate::sqlite::error::{ExtendedErrCode, PrimaryErrCode};
use crate::{
    ForeignKeyViolation, SqliteDataType,
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

/// Errors produced when mounting, migrating, or verifying a database module.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ModuleError {
    /// A module declared a dependency in `DEPENDS_ON` that is not installed.
    #[error("module `{module_id}` depends on missing module `{dependency_id}`")]
    DependencyMissing {
        /// Module being mounted.
        module_id: String,
        /// Missing dependency identifier.
        dependency_id: String,
    },

    /// Another module already owns the requested table prefix.
    #[error("module `{module_id}` prefix `{prefix}` conflicts with `{existing_module_id}`")]
    PrefixConflict {
        /// Module being mounted.
        module_id: String,
        /// Conflicting table prefix.
        prefix: String,
        /// Existing module that owns `prefix`.
        existing_module_id: String,
    },

    /// A module declared an invalid or reserved table prefix.
    #[error("module `{module_id}` has invalid prefix `{prefix}`: {reason}")]
    InvalidPrefix {
        /// Module identifier.
        module_id: String,
        /// Declared prefix.
        prefix: String,
        /// Explanation of why the prefix was rejected.
        reason: String,
    },

    /// A module created a schema object that violates prefix or table ownership
    /// rules.
    #[error("module `{module_id}` created disallowed {object_type} `{object_name}`: {reason}")]
    SchemaViolation {
        /// Module identifier.
        module_id: String,
        /// SQLite schema object type (`table`, `index`, `view`, or `trigger`).
        object_type: String,
        /// Created SQLite object name.
        object_name: String,
        /// Target table name reported by `sqlite_schema.tbl_name`, if any.
        target_table: Option<String>,
        /// Reason the object violated ownership rules.
        reason: String,
    },

    /// Installed module schema version is newer than the mounted module code.
    #[error(
        "module `{module_id}` downgrade rejected: installed version {installed_version} > target version {target_version}"
    )]
    Downgrade {
        /// Module identifier.
        module_id: String,
        /// Schema version recorded in `_musq_modules`.
        installed_version: u32,
        /// Schema version declared by the module.
        target_version: u32,
    },

    /// Module does not support migrating from `from_version` to `to_version`.
    #[error(
        "module `{module_id}` does not support migration from version {from_version} to {to_version}"
    )]
    MigrationUnsupported {
        /// Module identifier.
        module_id: String,
        /// Installed schema version.
        from_version: u32,
        /// Target schema version.
        to_version: u32,
    },

    /// Requested module is not installed in `_musq_modules`.
    #[error("module `{module_id}` is not installed")]
    NotInstalled {
        /// Module identifier.
        module_id: String,
    },

    /// Installed module schema version differs from the expected version.
    #[error(
        "module `{module_id}` version mismatch: installed {installed_version}, expected {expected_version}"
    )]
    VersionMismatch {
        /// Module identifier.
        module_id: String,
        /// Schema version recorded in `_musq_modules`.
        installed_version: u32,
        /// Expected schema version.
        expected_version: u32,
    },

    /// Installed module table prefix differs from the declared prefix.
    #[error(
        "module `{module_id}` prefix mismatch: installed `{installed_prefix}`, expected `{expected_prefix}`"
    )]
    PrefixMismatch {
        /// Module identifier.
        module_id: String,
        /// Table prefix recorded in `_musq_modules`.
        installed_prefix: String,
        /// Declared table prefix.
        expected_prefix: String,
    },

    /// `PRAGMA foreign_key_check` found violations after module install or
    /// migration.
    #[error(
        "module `{module_id}` left {} foreign key violation(s)",
        violations.len()
    )]
    ForeignKeyViolations {
        /// Module identifier.
        module_id: String,
        /// Foreign-key violations reported by SQLite.
        violations: Vec<ForeignKeyViolation>,
    },
}

/// Errors produced by [`crate::ReadOnlySandbox`] and [`crate::fleet_query`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SandboxError {
    /// Query was rejected by single-statement validation or the SQLite
    /// authorizer.
    #[error("sandbox query rejected: {0}")]
    Rejected(String),

    /// Query exceeded the configured statement timeout.
    #[error("sandbox query timed out: {0}")]
    Timeout(String),

    /// Query exceeded a SQLite runtime limit.
    #[error("sandbox query exceeded limit: {0}")]
    LimitExceeded(String),

    /// Query parameter could not be bound.
    #[error("invalid sandbox query parameter: {0}")]
    InvalidParameter(String),
}

/// Represents all the ways a method can fail within Musq.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Error returned from the database.
    #[error("{0}")]
    Sqlite(#[from] SqliteError),

    /// Error communicating with the database backend.
    #[error("error communicating with database: {0}")]
    Io(#[from] io::Error),

    /// Unexpected or invalid data encountered while communicating with the
    /// database.
    ///
    /// This should indicate there is a programming error in Musq or there
    /// is something corrupted with the connection to the database itself.
    #[error("encountered unexpected or invalid data: {0}")]
    Protocol(String),

    /// Invalid builder option or connection configuration.
    #[error("{0}")]
    Configuration(String),

    /// Invalid query composition or bind arguments.
    #[error("{0}")]
    Query(String),

    /// No rows returned by a query that expected to return at least one row.
    #[error("no rows returned by a query that expected to return at least one row")]
    RowNotFound,

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

    /// A [`Pool::acquire`] timed out due to connections not becoming available
    /// or because another task encountered too many errors while trying to
    /// open a new connection.
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

    /// An interrupted write rolled the explicit transaction back.
    ///
    /// The next `commit` or `rollback` on this connection returns this error
    /// instead of running SQL at depth zero. Later statements run normally.
    #[error("the transaction was aborted by an interrupt")]
    TransactionAborted,

    /// Attempted a write, mount, or migration on a read-only database host.
    #[error("cannot write to a read-only database")]
    ReadOnly,

    /// Database header (`application_id` or `user_version`) did not match host
    /// expectations.
    #[error("incompatible database format at `{}`: {reason}", path.display())]
    IncompatibleFormat {
        /// Database file path.
        path: PathBuf,
        /// Description of the header mismatch.
        reason: String,
    },

    /// Exported archive failed integrity verification.
    #[error("archive verification failed: {0}")]
    ArchiveVerification(String),

    /// Module mounting, migration, or discovery error.
    #[error(transparent)]
    Module(#[from] ModuleError),

    /// Read-only query sandbox error.
    #[error(transparent)]
    Sandbox(#[from] SandboxError),
}

impl Error {
    /// Return a reference to the inner SQLite error when this error came from
    /// SQLite.
    pub fn as_sqlite(&self) -> Option<&SqliteError> {
        match self {
            Self::Sqlite(error) => Some(error),
            _ => None,
        }
    }

    /// Return the primary and extended SQLite codes without consuming the
    /// error.
    pub fn sqlite_codes(&self) -> Option<(PrimaryErrCode, Option<ExtendedErrCode>)> {
        self.as_sqlite().map(SqliteError::codes)
    }

    /// Returns `true` if this error represents a busy SQLite database.
    pub fn is_busy(&self) -> bool {
        self.as_sqlite().is_some_and(SqliteError::is_busy)
    }

    /// Returns `true` if this error represents a SQLite unique-value conflict.
    pub fn is_unique_violation(&self) -> bool {
        self.as_sqlite()
            .is_some_and(SqliteError::is_unique_violation)
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(_: PoisonError<T>) -> Self {
        Self::WorkerCrashed
    }
}
