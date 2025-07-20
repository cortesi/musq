use crate::{Arguments, Statement};

/// A type that may be executed against a database connection.
///
/// Implemented for the following:
///
///  * [`&str`](std::str)
///  * [`Query`](super::query::Query)
///
pub trait Execute: Send + Sized {
    /// Gets the SQL that will be executed.
    fn sql(&self) -> &str;

    /// Gets the previously cached statement, if available.
    fn statement(&self) -> Option<&Statement>;

    /// Returns the arguments to be bound against the query string.
    ///
    /// Returning `None` for `Arguments` indicates to use a "simple" query protocol and to not
    /// prepare the query. Returning `Some(Default::default())` is an empty arguments object that
    /// will be prepared (and cached) before execution.
    fn take_arguments(&mut self) -> Option<Arguments>;
}

impl Execute for &str {
    fn sql(&self) -> &str {
        self
    }

    fn statement(&self) -> Option<&Statement> {
        None
    }

    fn take_arguments(&mut self) -> Option<Arguments> {
        None
    }
}
