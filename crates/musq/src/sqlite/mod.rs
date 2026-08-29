pub use arguments::Arguments;
pub use connection::{
    BackupReport, Connection, DbStatus, DbStatusKind, DeserializeMode, ForeignKeyViolation,
    IntegrityReport, InterruptHandle, SqliteRuntimeInfo, WalCheckpoint, WalCheckpointMode,
};
pub use error::SqliteError;
pub use function::FunctionFlags;
pub use hooks::{UpdateEvent, UpdateOp};
pub use type_info::SqliteDataType;
pub use value::Value;

/// Argument parsing and binding.
mod arguments;
/// SQLite connection handling.
mod connection;
/// SQLite error types and helpers.
pub mod error;
/// Raw FFI bindings.
mod ffi;
/// User-defined scalar functions and collations.
pub mod function;
/// Per-connection SQLite hooks.
pub mod hooks;
/// Prepared statement types and helpers.
pub mod statement;
/// SQLite type information utilities.
mod type_info;
/// SQLite value container and accessors.
pub mod value;
