use musq::{query, query_scalar};
use musq_test::connection;
use std::sync::Arc;

#[tokio::test]
async fn insert_and_select_arc_blob() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    conn.execute("CREATE TABLE blob_test (data BLOB)").await?;

    let data: Vec<u8> = vec![0x42; 16 * 1024];
    let arc = Arc::new(data.clone());

    query("INSERT INTO blob_test (data) VALUES (?)")
        .bind(Arc::clone(&arc))
        .execute(&mut conn)
        .await?;

    let returned: Arc<Vec<u8>> = query_scalar("SELECT data FROM blob_test")
        .fetch_one(&mut conn)
        .await?;

    assert_eq!(*returned, data);
    Ok(())
}
