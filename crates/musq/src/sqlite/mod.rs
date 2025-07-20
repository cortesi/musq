pub use arguments::Arguments;
pub use connection::Connection;
pub use error::SqliteError;
pub use statement::Statement;
pub use type_info::SqliteDataType;
pub use value::Value;

mod arguments;
mod connection;
pub mod error;
mod ffi;
pub mod statement;
mod type_info;
pub(crate) mod value;

/// Default number of times [`unlock_notify::wait`] is allowed to retry when a
/// statement is reset due to `SQLITE_LOCKED`.
///
/// [`ConnectionHandle::exec`] and [`StatementHandle::step`] use this constant to
/// limit how many unlock notification attempts will be made before returning
/// [`Error::UnlockNotify`].
pub(crate) const DEFAULT_MAX_RETRIES: usize = 5;
