pub use arguments::Arguments;
pub use connection::Connection;
pub use error::SqliteError;
pub use statement::Prepared;
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
/// Prepared statement types and helpers.
pub mod statement;
/// SQLite type information utilities.
mod type_info;
/// SQLite value container and accessors.
pub mod value;

/// Default number of times [`unlock_notify::wait`] is allowed to retry when a
/// statement is reset due to `SQLITE_LOCKED`.
///
/// [`ConnectionHandle::exec`] and [`StatementHandle::step`] use this constant to
/// limit how many unlock notification attempts will be made before returning
/// [`Error::UnlockNotify`].
pub const DEFAULT_MAX_RETRIES: usize = 5;
