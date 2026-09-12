//! Serialize, deserialize, and backup snapshots.

mod support;

#[cfg(test)]
mod tests {
    use musq::{DeserializeMode, Musq, query, query_scalar};

    use crate::support::{
        assert_configuration_contains, connection,
        db::{populated_connection, populated_pool, wal_pool},
    };

    #[tokio::test]
    async fn serialize_main_returns_a_database_image() -> anyhow::Result<()> {
        let empty = connection().await?;
        let empty_image = empty.serialize("main").await?;
        assert!(!empty_image.is_empty());

        let conn = populated_connection().await?;
        let image = conn.serialize("main").await?;
        assert!(image.len() > empty_image.len());

        let restored = connection().await?;
        restored
            .deserialize("main", image, DeserializeMode::Resizable)
            .await?;
        let names: Vec<String> = query_scalar("SELECT name FROM items ORDER BY id")
            .fetch_all(&restored)
            .await?;
        assert_eq!(names, ["one", "two"]);
        Ok(())
    }

    #[tokio::test]
    async fn serialize_rejects_schema_with_nul() -> anyhow::Result<()> {
        let conn = connection().await?;
        let err = conn.serialize("ma\0in").await.unwrap_err();
        assert_configuration_contains(err, "nul");
        Ok(())
    }

    #[tokio::test]
    async fn deserialize_round_trips_a_database() -> anyhow::Result<()> {
        let conn = populated_connection().await?;
        let image = conn.serialize("main").await?;

        let dest = connection().await?;
        dest.deserialize("main", image, DeserializeMode::Resizable)
            .await?;
        let names: Vec<String> = query_scalar("SELECT name FROM items ORDER BY id")
            .fetch_all(&dest)
            .await?;
        assert_eq!(names, ["one", "two"]);
        query("INSERT INTO items(name) VALUES ('three')")
            .execute(&dest)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn deserialize_refuses_an_open_transaction() -> anyhow::Result<()> {
        let mut conn = populated_connection().await?;
        let image = conn.serialize("main").await?;
        let tx = conn.begin().await?;
        let err = tx
            .deserialize("main", image, DeserializeMode::ReadOnly)
            .await
            .unwrap_err();
        assert_configuration_contains(err, "transaction is open");
        tx.rollback().await?;
        Ok(())
    }

    #[tokio::test]
    async fn deserialize_rejects_a_wal_image() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let pool = wal_pool(dir.path()).await?;
        let conn = pool.acquire().await?;
        let image = conn.serialize("main").await?;
        drop(conn);

        let dest = connection().await?;
        let err = dest
            .deserialize("main", image, DeserializeMode::ReadOnly)
            .await
            .unwrap_err();
        assert_configuration_contains(err, "WAL");
        Ok(())
    }

    #[tokio::test]
    async fn backup_to_path_copies_a_file_database() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("source.db");
        let dest = dir.path().join("copy.db");
        let pool = populated_pool(&source).await?;

        let conn = pool.acquire().await?;
        let report = conn.backup_to_path(&dest, 5).await?;
        assert!(report.pages > 0);
        assert_eq!(report.remaining, 0);
        drop(conn);

        query("INSERT INTO items(name) VALUES ('three')")
            .execute(&pool)
            .await?;

        let copy = Musq::new().open(&dest).await?;
        let names: Vec<String> = query_scalar("SELECT name FROM items ORDER BY id")
            .fetch_all(&copy)
            .await?;
        assert_eq!(names, ["one", "two"]);
        Ok(())
    }

    #[tokio::test]
    async fn backup_to_path_rejects_the_source_file() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("source.db");
        let pool = populated_pool(&source).await?;
        let conn = pool.acquire().await?;
        let err = conn.backup_to_path(&source, 1).await.unwrap_err();
        assert_configuration_contains(err, "same");
        Ok(())
    }
}
