//! Integration tests for SQLite runtime diagnostics and control APIs.

mod support;

#[cfg(test)]
mod tests {
    use musq::{DbStatusKind, Error, JournalMode, Musq, WalCheckpointMode, query, query_scalar};
    use tempdir::TempDir;

    use crate::support::connection;

    /// SQLite version bundled by libsqlite3-sys 0.38.1.
    const BUNDLED_SQLITE_VERSION: &str = "3.53.2";
    /// Numeric SQLite version bundled by libsqlite3-sys 0.38.1.
    const BUNDLED_SQLITE_VERSION_NUMBER: i32 = 3_053_002;

    #[tokio::test]
    async fn runtime_info_reports_bundled_sqlite_identity() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;

        let pool_info = pool.runtime_info().await?;
        assert_eq!(pool_info.version, BUNDLED_SQLITE_VERSION);
        assert_eq!(pool_info.version_number, BUNDLED_SQLITE_VERSION_NUMBER);
        assert!(!pool_info.source_id.is_empty());
        assert!(
            pool_info
                .compile_options
                .contains(&"ENABLE_FTS5".to_string()),
            "compile options missing ENABLE_FTS5: {:#?}",
            pool_info.compile_options
        );

        let conn = pool.acquire().await?;
        let conn_info = conn.runtime_info().await?;
        assert_eq!(conn_info.version, pool_info.version);
        assert_eq!(conn_info.version_number, pool_info.version_number);

        let _ = pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn db_status_reports_statement_cache_memory() -> anyhow::Result<()> {
        let conn = connection().await?;

        let before = conn.db_status(DbStatusKind::StatementUsed, false).await?;
        let value: i64 = query_scalar("SELECT ?")
            .bind(42_i64)
            .fetch_one(&conn)
            .await?;
        assert_eq!(value, 42);

        let after = conn.db_status(DbStatusKind::StatementUsed, false).await?;
        assert!(
            after.current > before.current,
            "expected StatementUsed to grow after preparing a cached statement; before={before:?}, after={after:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn wal_checkpoint_noop_reports_file_backed_wal_status() -> anyhow::Result<()> {
        let dir = TempDir::new("musq-wal-checkpoint")?;
        let path = dir.path().join("wal.db");
        let pool = Musq::new()
            .create_if_missing(true)
            .journal_mode(JournalMode::Wal)
            .open(&path)
            .await?;

        query("CREATE TABLE checkpoint_items(id INTEGER PRIMARY KEY, name TEXT)")
            .execute(&pool)
            .await?;
        query("INSERT INTO checkpoint_items(name) VALUES ('one'), ('two')")
            .execute(&pool)
            .await?;

        let checkpoint = pool
            .wal_checkpoint(Some("main"), WalCheckpointMode::Noop)
            .await?;
        let log_frames = checkpoint.log_frames.expect("WAL frame count");
        let checkpointed_frames = checkpoint
            .checkpointed_frames
            .expect("checkpointed WAL frame count");

        assert!(log_frames >= 0);
        assert!(checkpointed_frames >= 0);
        assert!(log_frames >= checkpointed_frames);

        let _ = pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn floating_point_text_digits_controls_text_rendering() -> anyhow::Result<()> {
        let pool = Musq::new()
            .floating_point_text_digits(4)
            .open_in_memory()
            .await?;

        let rendered: String = query_scalar("SELECT CAST(1.0 / 3.0 AS TEXT)")
            .fetch_one(&pool)
            .await?;
        assert_eq!(rendered, "0.3333");

        let error = Musq::new()
            .floating_point_text_digits(3)
            .open_in_memory()
            .await
            .unwrap_err();
        assert_protocol_contains(error, "between 4 and 23");

        let _ = pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn parser_depth_limit_is_reported_and_enforced() -> anyhow::Result<()> {
        let conn = musq::Connection::connect_with(&Musq::new().parser_depth_limit(4)).await?;
        assert_eq!(conn.parser_depth_limit().await?, 4);

        let query = nested_expression(200);
        let error = query_scalar::<i64>(query.as_str())
            .fetch_one(&conn)
            .await
            .unwrap_err();
        let sqlite_error = error.into_sqlite_error().expect("SQLite parser error");
        assert!(
            sqlite_error.message.contains("parser stack overflow")
                || sqlite_error.message.contains("Recursion limit")
                || sqlite_error.message.contains("too many"),
            "unexpected parser depth error: {sqlite_error:?}"
        );

        let error = Musq::new()
            .parser_depth_limit(0)
            .open_in_memory()
            .await
            .unwrap_err();
        assert_protocol_contains(error, "greater than zero");

        Ok(())
    }

    fn nested_expression(depth: usize) -> String {
        format!("SELECT {}1{}", "(".repeat(depth), ")".repeat(depth))
    }

    fn assert_protocol_contains(error: Error, expected: &str) {
        match error {
            Error::Protocol(message) => assert!(
                message.contains(expected),
                "protocol error {message:?} did not contain {expected:?}"
            ),
            other => panic!("expected protocol error containing {expected:?}, got {other:?}"),
        }
    }
}
