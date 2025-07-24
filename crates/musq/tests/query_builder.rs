use musq::{Error, QueryBuilder};

#[test]
fn push_values_empty_iterator_returns_error() {
    let mut builder = QueryBuilder::new();
    let result = builder.push_values(std::iter::empty::<i32>());
    match result {
        Err(Error::Protocol(msg)) => assert!(msg.contains("empty values")),
        other => panic!("expected protocol error, got {other:?}"),
    }
}

#[test]
fn push_idents_empty_iterator_returns_error() {
    let mut builder = QueryBuilder::new();
    let result = builder.push_idents(std::iter::empty::<&str>());
    match result {
        Err(Error::Protocol(msg)) => assert!(msg.contains("empty idents")),
        other => panic!("expected protocol error, got {other:?}"),
    }
}
