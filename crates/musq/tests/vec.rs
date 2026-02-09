//! Integration tests for sqlite-vec support.

#![cfg_attr(not(feature = "vec"), allow(missing_docs))]
#![cfg(feature = "vec")]

mod support;

#[cfg(test)]
mod tests {
    use musq::{FromRow, Musq, VecBit, VecF32, VecInt8};

    use crate::support::connection;

    #[tokio::test]
    async fn vec_extension_available_on_direct_connection() -> anyhow::Result<()> {
        let conn = connection().await?;
        let version: String = musq::query_scalar("SELECT vec_version()")
            .fetch_one(&conn)
            .await?;
        assert!(version.starts_with('v'));
        Ok(())
    }

    #[tokio::test]
    async fn vec_extension_available_on_pool_connection() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;
        let version: String = musq::query_scalar("SELECT vec_version()")
            .fetch_one(&pool)
            .await?;
        assert!(version.starts_with('v'));
        let _ = pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn vec0_knn_query_with_vecf32() -> anyhow::Result<()> {
        let conn = connection().await?;

        musq::query("CREATE VIRTUAL TABLE vec_items USING vec0(embedding float[3])")
            .execute(&conn)
            .await?;

        musq::query("INSERT INTO vec_items(rowid, embedding) VALUES (?, ?)")
            .bind(1_i64)
            .bind(VecF32(vec![1.0, 0.0, 0.0]))
            .execute(&conn)
            .await?;
        musq::query("INSERT INTO vec_items(rowid, embedding) VALUES (?, ?)")
            .bind(2_i64)
            .bind(VecF32(vec![0.0, 1.0, 0.0]))
            .execute(&conn)
            .await?;
        musq::query("INSERT INTO vec_items(rowid, embedding) VALUES (?, ?)")
            .bind(3_i64)
            .bind(VecF32(vec![0.0, 0.0, 1.0]))
            .execute(&conn)
            .await?;

        let rows: Vec<(i64, f64)> = musq::query_as(
            "SELECT rowid, distance \
             FROM vec_items \
             WHERE embedding MATCH ? \
             ORDER BY distance \
             LIMIT 2",
        )
        .bind(VecF32(vec![0.0, 1.0, 0.0]))
        .fetch_all(&conn)
        .await?;

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, 2);
        assert!(rows[0].1 <= rows[1].1);

        Ok(())
    }

    #[tokio::test]
    async fn vec_distance_functions_work() -> anyhow::Result<()> {
        let conn = connection().await?;

        let l2: f64 = musq::query_scalar("SELECT vec_distance_l2(?, ?)")
            .bind(VecF32(vec![1.0, 2.0, 3.0]))
            .bind(VecF32(vec![1.0, 2.0, 3.0]))
            .fetch_one(&conn)
            .await?;
        assert_eq!(l2, 0.0);

        let cosine: f64 = musq::query_scalar("SELECT vec_distance_cosine(?, ?)")
            .bind(VecF32(vec![1.0, 2.0, 3.0]))
            .bind(VecF32(vec![1.0, 2.0, 3.0]))
            .fetch_one(&conn)
            .await?;
        assert_eq!(cosine, 0.0);

        Ok(())
    }

    #[derive(Debug, PartialEq, FromRow)]
    struct EmbeddingRow {
        embedding: VecF32,
    }

    #[tokio::test]
    async fn from_row_with_vecf32() -> anyhow::Result<()> {
        let conn = connection().await?;

        musq::query("CREATE VIRTUAL TABLE vec_items_row USING vec0(embedding float[3])")
            .execute(&conn)
            .await?;
        musq::query("INSERT INTO vec_items_row(rowid, embedding) VALUES (?, ?)")
            .bind(1_i64)
            .bind(VecF32(vec![0.25, 0.5, 0.75]))
            .execute(&conn)
            .await?;

        let row: EmbeddingRow =
            musq::query_as("SELECT embedding FROM vec_items_row WHERE rowid = ?")
                .bind(1_i64)
                .fetch_one(&conn)
                .await?;
        assert_eq!(
            row,
            EmbeddingRow {
                embedding: VecF32(vec![0.25, 0.5, 0.75]),
            }
        );

        Ok(())
    }

    #[tokio::test]
    async fn subtype_sensitive_vector_wrappers() -> anyhow::Result<()> {
        let conn = connection().await?;

        let int8_type: String = musq::query_scalar("SELECT vec_type(vec_int8(?))")
            .bind(VecInt8(vec![-128, -1, 0, 127]))
            .fetch_one(&conn)
            .await?;
        assert_eq!(int8_type, "int8");

        let bit_type: String = musq::query_scalar("SELECT vec_type(vec_bit(?))")
            .bind(VecBit(vec![0b1010_1010, 0b0101_0101]))
            .fetch_one(&conn)
            .await?;
        assert_eq!(bit_type, "bit");

        let plain_type: String = musq::query_scalar("SELECT vec_type(?)")
            .bind(VecInt8(vec![-128, -1, 0, 127]))
            .fetch_one(&conn)
            .await?;
        assert_ne!(plain_type, "int8");

        Ok(())
    }
}
