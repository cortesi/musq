//! Integration tests for musq.

mod support;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use musq::{query, query_scalar};

    use crate::support::connection;

    #[tokio::test]
    async fn insert_and_select_arc_blob() -> anyhow::Result<()> {
        let conn = connection().await?;
        query("CREATE TABLE blob_test (data BLOB)")
            .execute(&conn)
            .await?;

        let data: Vec<u8> = vec![0x42; 16 * 1024];
        let arc = Arc::new(data.clone());

        query("INSERT INTO blob_test (data) VALUES (?)")
            .bind(Arc::clone(&arc))
            .execute(&conn)
            .await?;

        let returned: Arc<Vec<u8>> = query_scalar("SELECT data FROM blob_test")
            .fetch_one(&conn)
            .await?;

        assert_eq!(*returned, data);
        Ok(())
    }
}
