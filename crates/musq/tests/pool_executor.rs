use musq::{Connection, Executor, Musq, query, query_as};
use std::sync::Arc;

/// Test that Pool implements Executor and can be used interchangeably with Connection
#[tokio::test]
async fn test_pool_implements_executor() -> anyhow::Result<()> {
    let pool = Musq::new().open_in_memory().await?;

    // Setup test table using pool as executor
    query("CREATE TABLE test_executor (id INTEGER, value TEXT)")
        .execute(&pool)
        .await?;

    // Insert data using pool as executor
    query("INSERT INTO test_executor (id, value) VALUES (?, ?)")
        .bind(1)
        .bind("test_value")
        .execute(&pool)
        .await?;

    // Query data using pool as executor
    let (id, value): (i32, String) = query_as("SELECT id, value FROM test_executor WHERE id = ?")
        .bind(1)
        .fetch_one(&pool)
        .await?;

    assert_eq!(id, 1);
    assert_eq!(value, "test_value");

    Ok(())
}

/// Test that queries work the same whether using Pool, Connection, or PoolConnection
#[tokio::test]
async fn test_executor_interchangeability() -> anyhow::Result<()> {
    let pool = Musq::new().open_in_memory().await?;

    // Setup table using pool
    query("CREATE TABLE test_interop (id INTEGER, value TEXT)")
        .execute(&pool)
        .await?;

    // Test with Pool
    query("INSERT INTO test_interop (id, value) VALUES (?, ?)")
        .bind(1)
        .bind("from_pool")
        .execute(&pool)
        .await?;

    // Test with PoolConnection
    let pool_conn = pool.acquire().await?;
    query("INSERT INTO test_interop (id, value) VALUES (?, ?)")
        .bind(2)
        .bind("from_pool_conn")
        .execute(&pool_conn)
        .await?;

    // Test with standalone Connection
    let standalone_conn = Connection::connect_with(&Musq::new()).await?;
    // Note: This won't see the data from the pool since it's a different connection
    // But we can test that the API works the same
    query("CREATE TABLE test_standalone (id INTEGER, value TEXT)")
        .execute(&standalone_conn)
        .await?;

    query("INSERT INTO test_standalone (id, value) VALUES (?, ?)")
        .bind(3)
        .bind("from_standalone")
        .execute(&standalone_conn)
        .await?;

    // Verify pool data
    let pool_results: Vec<(i32, String)> =
        query_as("SELECT id, value FROM test_interop ORDER BY id")
            .fetch_all(&pool)
            .await?;

    assert_eq!(pool_results.len(), 2);
    assert_eq!(pool_results[0], (1, "from_pool".to_string()));
    assert_eq!(pool_results[1], (2, "from_pool_conn".to_string()));

    // Verify standalone connection data
    let standalone_result: (i32, String) =
        query_as("SELECT id, value FROM test_standalone WHERE id = ?")
            .bind(3)
            .fetch_one(&standalone_conn)
            .await?;

    assert_eq!(standalone_result, (3, "from_standalone".to_string()));

    Ok(())
}

/// Test that Pool can be used in generic functions that accept Executor
#[tokio::test]
async fn test_pool_in_generic_function() -> anyhow::Result<()> {
    async fn insert_and_count<E: for<'a> Executor<'a> + Send + Sync>(
        executor: &E,
        table: &str,
        value: &str,
    ) -> anyhow::Result<i64> {
        query(&format!("INSERT INTO {} (value) VALUES (?)", table))
            .bind(value)
            .execute(executor)
            .await?;

        let count: (i64,) = query_as(&format!("SELECT COUNT(*) FROM {}", table))
            .fetch_one(executor)
            .await?;

        Ok(count.0)
    }

    let pool = Musq::new().open_in_memory().await?;

    // Setup table
    query("CREATE TABLE test_generic (value TEXT)")
        .execute(&pool)
        .await?;

    // Test with pool
    let count1 = insert_and_count(&pool, "test_generic", "value1").await?;
    assert_eq!(count1, 1);

    let count2 = insert_and_count(&pool, "test_generic", "value2").await?;
    assert_eq!(count2, 2);

    // Test with pool connection
    let conn = pool.acquire().await?;
    let count3 = insert_and_count(&conn, "test_generic", "value3").await?;
    assert_eq!(count3, 3);

    Ok(())
}

/// Test concurrent usage of Pool as Executor
#[tokio::test]
async fn test_pool_executor_concurrent() -> anyhow::Result<()> {
    let pool = Arc::new(Musq::new().max_connections(5).open_in_memory().await?);

    // Setup table
    query("CREATE TABLE test_concurrent_pool (id INTEGER, thread_id INTEGER)")
        .execute(&*pool)
        .await?;

    // Run concurrent operations using Pool as Executor
    let mut handles = vec![];
    for thread_id in 0..10 {
        let pool_clone = Arc::clone(&pool);
        let handle = tokio::spawn(async move {
            for i in 0..3 {
                query("INSERT INTO test_concurrent_pool (id, thread_id) VALUES (?, ?)")
                    .bind(thread_id * 3 + i)
                    .bind(thread_id)
                    .execute(&*pool_clone)
                    .await
                    .unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    futures::future::join_all(handles).await;

    // Verify all data was inserted
    let count: (i64,) = query_as("SELECT COUNT(*) FROM test_concurrent_pool")
        .fetch_one(&*pool)
        .await?;

    assert_eq!(count.0, 30); // 10 threads * 3 inserts each

    Ok(())
}
