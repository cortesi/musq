//! Serialize, deserialize, and backup snapshots.

#[cfg(test)]
mod tests {
    use musq::{Connection, Musq, query, query_scalar};

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
}
