use musq::{JournalMode, Musq, query_scalar};

#[tokio::test]
async fn open_in_memory_with_journal_mode_memory() -> anyhow::Result<()> {
    let options = Musq::new().journal_mode(JournalMode::Memory);

    let pool = options.open_in_memory().await?;
    let conn = pool.acquire().await?;

    let mode: String = query_scalar("PRAGMA journal_mode").fetch_one(&conn).await?;
    assert_eq!(mode.to_uppercase(), "MEMORY");

    Ok(())
}
