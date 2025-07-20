use musq::{Musq, query, query_scalar};
use musq_test::connection;
use tokio::time::{Duration, Instant, sleep};

#[tokio::test]
async fn basic_statement_flow() -> anyhow::Result<()> {
    let mut conn = connection().await?;

    conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)")
        .await?;

    let stmt = conn.prepare("INSERT INTO t (val) VALUES (?1)").await?;
    stmt.query().bind("hello").execute(&mut conn).await?;
    drop(stmt);

    let count: i64 = query_scalar("SELECT COUNT(*) FROM t")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(count, 1);

    Ok(())
}

#[tokio::test]
async fn retry_on_busy_lock() -> anyhow::Result<()> {
    let pool = Musq::new().max_connections(2).open_in_memory().await?;
    let mut c1 = pool.acquire().await?;
    let mut c2 = pool.acquire().await?;

    c1.execute("CREATE TABLE t (val TEXT)").await?;

    query("BEGIN IMMEDIATE").execute(&mut c1).await?;

    let start = Instant::now();
    let insert = tokio::spawn(async move {
        query("INSERT INTO t (val) VALUES ('foo')")
            .execute(&mut c2)
            .await
    });

    sleep(Duration::from_millis(100)).await;
    query("COMMIT").execute(&mut c1).await?;

    insert.await??;
    assert!(start.elapsed() >= Duration::from_millis(100));

    let mut conn = pool.acquire().await?;
    let count: i64 = query_scalar("SELECT COUNT(*) FROM t")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(count, 1);

    Ok(())
}
