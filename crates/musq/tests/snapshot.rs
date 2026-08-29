//! Serialize, deserialize, and backup snapshots.

#[cfg(test)]
mod tests {
    use musq::{Connection, DeserializeMode, Error, JournalMode, Musq, query, query_scalar};
    use tempdir::TempDir;

    async fn populated() -> anyhow::Result<Connection> {
        let conn = Connection::connect_with(&Musq::new()).await?;
        query("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .execute(&conn)
            .await?;
        query("INSERT INTO t (id, name) VALUES (1, 'one'), (2, 'two')")
            .execute(&conn)
            .await?;
        Ok(conn)
    }

    #[tokio::test]
    async fn serialize_main_returns_a_database_image() -> anyhow::Result<()> {
        let empty = Connection::connect_with(&Musq::new()).await?;
        let empty_image = empty.serialize("main").await?;
        assert!(!empty_image.is_empty());

        let conn = populated().await?;
        let image = conn.serialize("main").await?;
        assert!(image.len() >= empty_image.len());
        let count: i64 = query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(&conn)
            .await?;
        assert_eq!(count, 2);
        Ok(())
    }

    #[tokio::test]
    async fn serialize_rejects_schema_with_nul() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;
        let err = conn.serialize("ma\0in").await.unwrap_err();
        assert!(matches!(err, musq::Error::Configuration(_)));
        Ok(())
    }

    #[tokio::test]
    async fn deserialize_round_trips_a_database() -> anyhow::Result<()> {
        let conn = populated().await?;
        let image = conn.serialize("main").await?;

        let dest = Connection::connect_with(&Musq::new()).await?;
        dest.deserialize("main", image, DeserializeMode::Resizable)
            .await?;
        let names: Vec<String> = query_scalar("SELECT name FROM t ORDER BY id")
            .fetch_all(&dest)
            .await?;
        assert_eq!(names, ["one", "two"]);
        query("INSERT INTO t (id, name) VALUES (3, 'three')")
            .execute(&dest)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn deserialize_refuses_an_open_transaction() -> anyhow::Result<()> {
        let mut conn = populated().await?;
        let image = conn.serialize("main").await?;
        let tx = conn.begin().await?;
        let err = tx
            .deserialize("main", image, DeserializeMode::ReadOnly)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Configuration(_)));
        tx.rollback().await?;
        Ok(())
    }

    #[tokio::test]
    async fn deserialize_rejects_a_wal_image() -> anyhow::Result<()> {
        let dir = TempDir::new("musq-deserialize-wal")?;
        let path = dir.path().join("wal.db");
        let pool = Musq::new()
            .create_if_missing(true)
            .journal_mode(JournalMode::Wal)
            .open(&path)
            .await?;
        query("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await?;
        let conn = pool.acquire().await?;
        let image = conn.serialize("main").await?;
        drop(conn);

        let dest = Connection::connect_with(&Musq::new()).await?;
        let err = dest
            .deserialize("main", image, DeserializeMode::ReadOnly)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Configuration(ref msg) if msg.contains("WAL")),
            "got {err:?}"
        );
        Ok(())
    }
}
