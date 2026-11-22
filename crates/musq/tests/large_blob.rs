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

    #[tokio::test]
    async fn insert_and_select_multi_megabyte_blobs() -> anyhow::Result<()> {
        let conn = connection().await?;
        query("CREATE TABLE large_blobs (id INTEGER PRIMARY KEY, data BLOB NOT NULL)")
            .execute(&conn)
            .await?;

        let size = 2 * 1024 * 1024 + 137;
        let data: Vec<u8> = (0..size)
            .map(|idx| ((idx * 31 + 17) & 0xff) as u8)
            .collect();
        let arc = Arc::new(data.clone());

        query("INSERT INTO large_blobs (id, data) VALUES (?, ?)")
            .bind(1_i64)
            .bind(&data)
            .execute(&conn)
            .await?;
        query("INSERT INTO large_blobs (id, data) VALUES (?, ?)")
            .bind(2_i64)
            .bind(Arc::clone(&arc))
            .execute(&conn)
            .await?;

        let returned_vec: Vec<u8> = query_scalar("SELECT data FROM large_blobs WHERE id = 1")
            .fetch_one(&conn)
            .await?;
        let returned_arc: Arc<Vec<u8>> = query_scalar("SELECT data FROM large_blobs WHERE id = 2")
            .fetch_one(&conn)
            .await?;

        assert_eq!(returned_vec.len(), size);
        assert_eq!(returned_vec, data);
        assert_eq!(*returned_arc, data);
        Ok(())
    }
}
