use musq::{Error, ExtendedErrCode, PrimaryErrCode, query};
use musq_test::tdb;

#[tokio::test]
async fn it_fails_with_unique_violation() -> anyhow::Result<()> {
    let mut conn = tdb().await?;
    let mut tx = conn.begin().await?;

    let res: Result<_, Error> = query("INSERT INTO tweet VALUES (1, 'Foo', true, 1);")
        .execute(&mut tx)
        .await;
    let err = res.unwrap_err();

    let err = err.into_sqlite_error().unwrap();

    assert_eq!(err.primary, PrimaryErrCode::Constraint);
    assert_eq!(err.extended, ExtendedErrCode::ConstraintPrimaryKey);

    Ok(())
}

#[tokio::test]
async fn it_fails_with_foreign_key_violation() -> anyhow::Result<()> {
    let mut conn = tdb().await?;
    let mut tx = conn.begin().await?;

    let res: Result<_, Error> =
        query("INSERT INTO tweet_reply (id, tweet_id, text) VALUES (2, 2, 'Reply!');")
            .execute(&mut tx)
            .await;
    let err = res.unwrap_err();

    let err = err.into_sqlite_error().unwrap();

    assert_eq!(err.primary, PrimaryErrCode::Constraint);
    assert_eq!(err.extended, ExtendedErrCode::ConstraintForeignKey);

    Ok(())
}

#[tokio::test]
async fn it_fails_with_not_null_violation() -> anyhow::Result<()> {
    let mut conn = tdb().await?;
    let mut tx = conn.begin().await?;

    let res: Result<_, Error> = query("INSERT INTO tweet (text) VALUES (null);")
        .execute(&mut tx)
        .await;
    let err = res.unwrap_err();

    let err = err.into_sqlite_error().unwrap();

    assert_eq!(err.primary, PrimaryErrCode::Constraint);
    assert_eq!(err.extended, ExtendedErrCode::ConstraintNotNull);

    Ok(())
}

#[tokio::test]
async fn it_fails_with_check_violation() -> anyhow::Result<()> {
    let mut conn = tdb().await?;
    let mut tx = conn.begin().await?;

    let res: Result<_, Error> = query("INSERT INTO products VALUES (1, 'Product 1', 0);")
        .execute(&mut tx)
        .await;
    let err = res.unwrap_err();

    let err = err.into_sqlite_error().unwrap();

    assert_eq!(err.primary, PrimaryErrCode::Constraint);
    assert_eq!(err.extended, ExtendedErrCode::ConstraintCheck);

    Ok(())
}
