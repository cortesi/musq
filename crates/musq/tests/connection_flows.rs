//! Integration tests for musq.

mod support;

#[cfg(test)]
mod tests {
    use musq::{Musq, query, query_scalar};
    use tokio::time::{Duration, Instant, sleep};

    use crate::support::connection;

    #[tokio::test]
    async fn basic_statement_flow() -> anyhow::Result<()> {
        let conn = connection().await?;

        query("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)")
            .execute(&conn)
            .await?;

        let stmt = conn.prepare("INSERT INTO t (val) VALUES (?1)").await?;
        stmt.query().bind("hello").execute(&conn).await?;
        drop(stmt);

        let count: i64 = query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(&conn)
            .await?;
        assert_eq!(count, 1);

        Ok(())
    }

    #[tokio::test]
    async fn retry_on_busy_lock() -> anyhow::Result<()> {
        let pool = Musq::new().max_connections(2).open_in_memory().await?;
        let c1 = pool.acquire().await?;
        let c2 = pool.acquire().await?;

        query("CREATE TABLE t (val TEXT)").execute(&c1).await?;

        query("BEGIN IMMEDIATE").execute(&c1).await?;

        let start = Instant::now();
        let insert = tokio::spawn(async move {
            query("INSERT INTO t (val) VALUES ('foo')")
                .execute(&c2)
                .await
        });

        sleep(Duration::from_millis(100)).await;
        query("COMMIT").execute(&c1).await?;

        insert.await??;
        assert!(start.elapsed() >= Duration::from_millis(100));

        let conn = pool.acquire().await?;
        let count: i64 = query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(&conn)
            .await?;
        assert_eq!(count, 1);

        Ok(())
    }
}
