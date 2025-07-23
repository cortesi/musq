use musq::{Execute, insert_into, query_scalar};
use musq_test::connection;

#[tokio::test]
async fn no_values_query() -> anyhow::Result<()> {
    let builder = insert_into("users");
    assert!(builder.query().is_err());
    Ok(())
}

#[tokio::test]
async fn no_values_execute() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    musq::query("CREATE TABLE users (id INTEGER)")
        .execute(&conn)
        .await?;
    let res = insert_into("users").execute(&conn).await;
    assert!(res.is_err());
    Ok(())
}

#[tokio::test]
async fn single_value_insert() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    musq::query("CREATE TABLE t (id INTEGER)")
        .execute(&conn)
        .await?;
    let query = insert_into("t").value("id", 5).query()?;
    assert_eq!(query.sql(), "INSERT INTO \"t\" (\"id\") VALUES (?)");
    insert_into("t").value("id", 5).execute(&conn).await?;
    let id: i32 = query_scalar("SELECT id FROM t")
        .fetch_one(&conn)
        .await?;
    assert_eq!(id, 5);
    Ok(())
}

#[tokio::test]
async fn multiple_values_insert() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    musq::query("CREATE TABLE stuff (a INTEGER, b TEXT, c BOOLEAN, d BLOB)")
        .execute(&conn)
        .await?;
    insert_into("stuff")
        .value("a", 1_i32)
        .value("b", "hi")
        .value("c", true)
        .value("d", vec![1u8, 2, 3])
        .execute(&conn)
        .await?;
    let row: (i32, String, bool, Vec<u8>) = musq::query_as("SELECT a, b, c, d FROM stuff")
        .fetch_one(&conn)
        .await?;
    assert_eq!(row.0, 1);
    assert_eq!(row.1, "hi");
    assert!(row.2);
    assert_eq!(row.3, vec![1u8, 2, 3]);
    Ok(())
}

#[tokio::test]
async fn quoted_identifiers() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    musq::query("CREATE TABLE \"user-data\" (\"from-column\" INTEGER)")
        .execute(&conn)
        .await?;
    let q = insert_into("user-data").value("from-column", 10).query()?;
    assert_eq!(
        q.sql(),
        "INSERT INTO \"user-data\" (\"from-column\") VALUES (?)"
    );
    q.execute(&conn).await?;
    let val: i32 = query_scalar("SELECT \"from-column\" FROM \"user-data\"")
        .fetch_one(&conn)
        .await?;
    assert_eq!(val, 10);
    Ok(())
}

#[tokio::test]
async fn execute_on_pool() -> anyhow::Result<()> {
    let pool = musq::Musq::new().open_in_memory().await?;
    pool.execute(musq::query("CREATE TABLE pool_t (id INTEGER)"))
        .await?;
    insert_into("pool_t")
        .value("id", 1)
        .execute_on_pool(&pool)
        .await?;
    let mut conn = pool.acquire().await?;
    let id: i32 = query_scalar("SELECT id FROM pool_t")
        .fetch_one(&conn)
        .await?;
    assert_eq!(id, 1);
    Ok(())
}

#[tokio::test]
async fn transaction_insert() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    musq::query("CREATE TABLE tx_t (id INTEGER)")
        .execute(&conn)
        .await?;
    let mut tx = conn.begin().await?;
    insert_into("tx_t").value("id", 7).execute(&tx).await?;
    tx.commit().await?;
    drop(tx);
    let id: i32 = query_scalar("SELECT id FROM tx_t")
        .fetch_one(&conn)
        .await?;
    assert_eq!(id, 7);
    Ok(())
}
