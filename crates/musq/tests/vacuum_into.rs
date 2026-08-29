//! Integration tests for safe SQLite database copies.

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    use std::{io, path::Path};

    use musq::{Error, Musq, query, query_scalar};
    use tempdir::TempDir;

    #[tokio::test]
    async fn vacuum_into_creates_an_independent_copy() -> anyhow::Result<()> {
        let dir = TempDir::new("musq-vacuum-into")?;
        let source = dir.path().join("source.db");
        let destination = dir.path().join("copy.db");
        let pool = source_pool(&source).await?;

        pool.vacuum_into(&destination).await?;
        query("INSERT INTO items(name) VALUES ('after-copy')")
            .execute(&pool)
            .await?;

        let copy = Musq::new().open(&destination).await?;
        let source_count: i64 = query_scalar("SELECT COUNT(*) FROM items")
            .fetch_one(&pool)
            .await?;
        let copy_count: i64 = query_scalar("SELECT COUNT(*) FROM items")
            .fetch_one(&copy)
            .await?;

        assert_eq!(source_count, 3);
        assert_eq!(copy_count, 2);
        let _ = pool.close().await;
        let _ = copy.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn vacuum_into_accepts_a_quoted_path() -> anyhow::Result<()> {
        let dir = TempDir::new("musq-vacuum-quoted")?;
        let source = dir.path().join("source.db");
        let destination = dir.path().join("snapshot's copy.db");
        let pool = source_pool(&source).await?;

        pool.vacuum_into(&destination).await?;

        let copy = Musq::new().open(&destination).await?;
        let name: String = query_scalar("SELECT name FROM items WHERE id = 1")
            .fetch_one(&copy)
            .await?;
        assert_eq!(name, "one");
        let _ = pool.close().await;
        let _ = copy.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn vacuum_into_rejects_an_existing_non_empty_destination() -> anyhow::Result<()> {
        let dir = TempDir::new("musq-vacuum-existing")?;
        let source = dir.path().join("source.db");
        let destination = dir.path().join("existing.db");
        let pool = source_pool(&source).await?;
        let existing = Musq::new()
            .create_if_missing(true)
            .open(&destination)
            .await?;
        query("CREATE TABLE existing(id INTEGER PRIMARY KEY)")
            .execute(&existing)
            .await?;
        let _ = existing.close().await;

        let error = pool.vacuum_into(&destination).await.unwrap_err();
        assert_sqlite_message(error, "output file already exists");

        let _ = pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn vacuum_into_reports_sqlite_destination_failures() -> anyhow::Result<()> {
        let dir = TempDir::new("musq-vacuum-failure")?;
        let source = dir.path().join("source.db");
        let destination = dir.path().join("missing").join("copy.db");
        let pool = source_pool(&source).await?;

        let error = pool.vacuum_into(&destination).await.unwrap_err();
        assert!(matches!(error, Error::Sqlite(_)), "{error:?}");

        let _ = pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn vacuum_into_rejects_a_nul_byte() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;

        let error = pool.vacuum_into(Path::new("copy\0.db")).await.unwrap_err();
        assert_invalid_path(error, "must not contain nul bytes");

        let _ = pool.close().await;
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn vacuum_into_rejects_non_utf8() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;
        let destination = OsString::from_vec(vec![b'c', b'o', b'p', b'y', 0xff]);

        let error = pool.vacuum_into(destination).await.unwrap_err();
        assert_invalid_path(error, "must be valid UTF-8");

        let _ = pool.close().await;
        Ok(())
    }

    async fn source_pool(path: &Path) -> anyhow::Result<musq::Pool> {
        let pool = Musq::new().create_if_missing(true).open(path).await?;
        query("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .execute(&pool)
            .await?;
        query("INSERT INTO items(name) VALUES ('one'), ('two')")
            .execute(&pool)
            .await?;
        Ok(pool)
    }

    fn assert_invalid_path(error: Error, expected: &str) {
        match error {
            Error::Io(error) => {
                assert_eq!(error.kind(), io::ErrorKind::InvalidData);
                assert!(error.to_string().contains(expected), "{error:?}");
            }
            other => panic!("expected invalid path error, got {other:?}"),
        }
    }

    fn assert_sqlite_message(error: Error, expected: &str) {
        match error {
            Error::Sqlite(error) => {
                assert!(error.message.contains(expected), "{:?}", error.message)
            }
            other => panic!("expected SQLite error, got {other:?}"),
        }
    }
}
