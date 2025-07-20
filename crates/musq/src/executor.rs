use crate::Arguments;

// Private module that defines the `Sealed` trait used to prevent external
// implementations of [`Execute`].
mod sealed {
    /// Prevent downstream implementations of [`Execute`].
    pub trait Sealed {}

    impl Sealed for &str {}
    impl Sealed for crate::query::Query {}
    impl<F> Sealed for crate::query::Map<F> {}
}

/// A type that may be executed against a database connection.
///
/// This trait is **sealed** and cannot be implemented outside of this crate.
///
/// Implemented for the following:
///
///  * [`&str`](std::str)
///  * [`Query`](super::query::Query)
///  * [`Map<F>`](super::query::Map)
///
pub trait Execute: sealed::Sealed + Send + Sized {
    /// Gets the SQL that will be executed.
    fn sql(&self) -> &str;

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

    fn take_arguments(&mut self) -> Option<Arguments> {
        None
    }
}
