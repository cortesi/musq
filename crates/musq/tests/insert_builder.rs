use musq::{Execute, InsertInto, query_scalar};
use musq_test::connection;

#[tokio::test]
async fn no_values_query() -> anyhow::Result<()> {
    let builder = InsertInto("users");
    assert!(builder.query().is_err());
    Ok(())
}

#[tokio::test]
async fn no_values_execute() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    conn.execute("CREATE TABLE users (id INTEGER)").await?;
    let res = InsertInto("users").execute(&mut conn).await;
    assert!(res.is_err());
    Ok(())
}

#[tokio::test]
async fn single_value_insert() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    conn.execute("CREATE TABLE t (id INTEGER)").await?;
    let query = InsertInto("t").value("id", 5).query()?;
    assert_eq!(query.sql(), "INSERT INTO \"t\" (\"id\") VALUES (?)");
    InsertInto("t").value("id", 5).execute(&mut conn).await?;
    let id: i32 = query_scalar("SELECT id FROM t")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(id, 5);
    Ok(())
}

#[tokio::test]
async fn multiple_values_insert() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    conn.execute("CREATE TABLE stuff (a INTEGER, b TEXT, c BOOLEAN, d BLOB)")
        .await?;
    InsertInto("stuff")
        .value("a", 1_i32)
        .value("b", "hi")
        .value("c", true)
        .value("d", vec![1u8, 2, 3])
        .execute(&mut conn)
        .await?;
    let row: (i32, String, bool, Vec<u8>) = musq::query_as("SELECT a, b, c, d FROM stuff")
        .fetch_one(&mut conn)
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
    conn.execute("CREATE TABLE \"user-data\" (\"from-column\" INTEGER)")
        .await?;
    let q = InsertInto("user-data").value("from-column", 10).query()?;
    assert_eq!(
        q.sql(),
        "INSERT INTO \"user-data\" (\"from-column\") VALUES (?)"
    );
    q.execute(&mut conn).await?;
    let val: i32 = query_scalar("SELECT \"from-column\" FROM \"user-data\"")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(val, 10);
    Ok(())
}

#[tokio::test]
async fn execute_on_pool() -> anyhow::Result<()> {
    let pool = musq::Musq::new().open_in_memory().await?;
    pool.execute(musq::query("CREATE TABLE pool_t (id INTEGER)"))
        .await?;
    InsertInto("pool_t")
        .value("id", 1)
        .execute_on_pool(&pool)
        .await?;
    let mut conn = pool.acquire().await?;
    let id: i32 = query_scalar("SELECT id FROM pool_t")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(id, 1);
    Ok(())
}

#[tokio::test]
async fn transaction_insert() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    conn.execute("CREATE TABLE tx_t (id INTEGER)").await?;
    let mut tx = conn.begin().await?;
    InsertInto("tx_t").value("id", 7).execute(&mut tx).await?;
    tx.commit().await?;
    drop(tx);
    let id: i32 = query_scalar("SELECT id FROM tx_t")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(id, 7);
    Ok(())
}
