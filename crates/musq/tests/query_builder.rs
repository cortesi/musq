//! Integration tests for musq.

#[cfg(test)]
mod tests {
    use std::iter::empty;

    use musq::{Error, Execute, QueryBuilder};

    #[test]
    fn push_values_empty_iterator_returns_error() {
        let mut builder = QueryBuilder::new();
        let result = builder.push_values(empty::<i32>());
        match result {
            Err(Error::Protocol(msg)) => assert!(msg.contains("empty values")),
            other => panic!("expected protocol error, got {other:?}"),
        }
    }

    #[test]
    fn push_idents_empty_iterator_returns_error() {
        let mut builder = QueryBuilder::new();
        let result = builder.push_idents(empty::<&str>());
        match result {
            Err(Error::Protocol(msg)) => assert!(msg.contains("empty idents")),
            other => panic!("expected protocol error, got {other:?}"),
        }
    }

    #[test]
    fn push_bind_named_normalizes_prefixed_names() -> anyhow::Result<()> {
        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT ");
        builder.push_bind_named(":foo", &1_i32)?;
        let query = builder.build();
        assert_eq!(query.sql(), "SELECT :foo");
        Ok(())
    }
}
