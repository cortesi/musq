//! Integration tests for the bundled SQLite runtime policy.

#![cfg_attr(not(test), allow(missing_docs))]

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use musq::{Musq, query_as, query_scalar};

    /// SQLite version bundled by libsqlite3-sys 0.38.1.
    const BUNDLED_SQLITE_VERSION: &str = "3.53.2";

    #[tokio::test]
    async fn bundled_sqlite_version_and_compile_options_are_active() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;

        let version: String = query_scalar("SELECT sqlite_version()")
            .fetch_one(&pool)
            .await?;
        assert_eq!(version, BUNDLED_SQLITE_VERSION);

        let options: Vec<(String,)> = query_as("PRAGMA compile_options").fetch_all(&pool).await?;
        let options: BTreeSet<String> = options.into_iter().map(|(option,)| option).collect();

        for option in [
            "ENABLE_COLUMN_METADATA",
            "ENABLE_FTS5",
            "ENABLE_RTREE",
            "ENABLE_UNLOCK_NOTIFY",
            "THREADSAFE=1",
        ] {
            assert!(
                options.contains(option),
                "bundled SQLite compile option {option} missing from {options:#?}"
            );
        }

        let _ = pool.close().await;
        Ok(())
    }
}
