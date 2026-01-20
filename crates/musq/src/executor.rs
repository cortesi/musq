use crate::Arguments;

/// Private module that defines the `Sealed` trait used to prevent external
/// implementations of [`Execute`].
mod sealed {
    use crate::query::{Map, Query};

    /// Prevent downstream implementations of [`Execute`].
    pub trait Sealed {}

    impl Sealed for &str {}
    impl Sealed for Query {}
    impl<F> Sealed for Map<F> {}
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
    fn arguments(&mut self) -> Option<Arguments>;
}

impl Execute for &str {
    fn sql(&self) -> &str {
        self
    }

    fn arguments(&mut self) -> Option<Arguments> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{Arguments, Value, executor::Execute, query_with};

    #[test]
    fn query_arguments_are_taken_without_clone() {
        let buf = vec![42_u8; 1024 * 1024];
        let original_ptr = buf.as_ptr();

        let arguments = Arguments {
            values: vec![Value::Blob {
                value: buf,
                type_info: None,
            }],
            named: HashMap::new(),
        };

        let mut query = query_with("SELECT ?1", arguments);

        let args = query.arguments().expect("expected arguments");

        match &args.values[0] {
            Value::Blob { value, .. } => assert_eq!(value.as_ptr(), original_ptr),
            other => panic!("expected blob, got {other:?}"),
        }

        assert!(query.arguments.is_none());
    }
}
