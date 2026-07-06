//! Integration tests for musq.

#[cfg(test)]
mod tests {
    use musq::{Error, JournalMode, Musq, query_scalar};

    #[tokio::test]
    async fn open_in_memory_with_journal_mode_memory() -> anyhow::Result<()> {
        let options = Musq::new().journal_mode(JournalMode::Memory);

        let pool = options.open_in_memory().await?;
        let conn = pool.acquire().await?;

        let mode: String = query_scalar("PRAGMA journal_mode").fetch_one(&conn).await?;
        assert_eq!(mode.to_uppercase(), "MEMORY");

        Ok(())
    }

    #[tokio::test]
    async fn max_connections_zero_is_rejected() {
        let err = Musq::new()
            .max_connections(0)
            .open_in_memory()
            .await
            .expect_err("max_connections(0) should be rejected");

        match err {
            Error::Protocol(msg) => assert!(msg.contains("max_connections")),
            other => panic!("expected protocol error, got {other:?}"),
        }
    }
}
