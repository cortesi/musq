use musq::{Error, Value, query_as};
use musq_macros::*;
use musq_test::connection;

#[derive(Debug, FromRow, PartialEq)]
struct Foo {
    #[musq(try_from = "i64")]
    value: u64,
}

#[tokio::test]
async fn try_from_failure_maps_error() -> anyhow::Result<()> {
    let mut conn = connection().await?;

    let res: musq::Result<Foo> = query_as::<Foo>("SELECT -1 as value")
        .fetch_one(&mut conn)
        .await;

    let err = res.expect_err("expected failure");
    if let Error::ColumnDecode {
        column_name, value, ..
    } = err
    {
        assert_eq!(column_name, "value");
        match value {
            Value::Integer { value, .. } => assert_eq!(value, -1),
            other => panic!("unexpected value: {other:?}"),
        }
    } else {
        panic!("unexpected error: {err:?}");
    }

    Ok(())
}
