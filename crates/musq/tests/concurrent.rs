//! Integration tests for musq.

mod support;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::support::connection;
    use futures::future::join_all;
    use musq::{query, query_as};

    /// Test that multiple concurrent reads work without blocking each other
    #[tokio::test]
    async fn test_concurrent_reads() -> anyhow::Result<()> {
        let conn = Arc::new(connection().await?);

        // Setup test data
        query("CREATE TABLE test_concurrent_reads (id INTEGER, value TEXT)")
            .execute(&*conn)
            .await?;
        for i in 0..10 {
            query("INSERT INTO test_concurrent_reads (id, value) VALUES (?, ?)")
                .bind(i)
                .bind(format!("value_{i}"))
                .execute(&*conn)
                .await?;
        }

        // Run multiple concurrent queries
        let mut handles = vec![];
        for i in 0..5 {
            let conn_clone = Arc::clone(&conn);
            let handle = tokio::spawn(async move {
                let rows: Vec<(i32, String)> =
                    query_as("SELECT id, value FROM test_concurrent_reads WHERE id >= ?")
                        .bind(i)
                        .fetch_all(&*conn_clone)
                        .await
                        .unwrap();
                rows.len()
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = join_all(handles).await;

        // Verify all tasks completed successfully
        for result in results {
            assert!(result? > 0);
        }

        Ok(())
    }

    /// Test that concurrent execute operations work correctly
    #[tokio::test]
    async fn test_concurrent_executes() -> anyhow::Result<()> {
        let conn = Arc::new(connection().await?);

        // Setup test table
        query("CREATE TABLE test_concurrent_executes (id INTEGER, thread_id INTEGER)")
            .execute(&*conn)
            .await?;

        // Run multiple concurrent inserts
        let mut handles = vec![];
        for thread_id in 0..10 {
            let conn_clone = Arc::clone(&conn);
            let handle = tokio::spawn(async move {
                for i in 0..5 {
                    query("INSERT INTO test_concurrent_executes (id, thread_id) VALUES (?, ?)")
                        .bind(thread_id * 5 + i)
                        .bind(thread_id)
                        .execute(&*conn_clone)
                        .await
                        .unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for result in join_all(handles).await {
            result?;
        }

        // Verify all data was inserted
        let count: (i64,) = query_as("SELECT COUNT(*) FROM test_concurrent_executes")
            .fetch_one(&*conn)
            .await?;
        assert_eq!(count.0, 50);

        Ok(())
    }

    /// Test that concurrent prepared statement usage works
    #[tokio::test]
    async fn test_concurrent_prepared_statements() -> anyhow::Result<()> {
        let conn = Arc::new(connection().await?);

        // Setup test table
        query("CREATE TABLE test_concurrent_prepared (id INTEGER, data TEXT)")
            .execute(&*conn)
            .await?;

        // Insert some test data
        for i in 0..100 {
            query("INSERT INTO test_concurrent_prepared (id, data) VALUES (?, ?)")
                .bind(i)
                .bind(format!("data_{i}"))
                .execute(&*conn)
                .await?;
        }

        // Run multiple concurrent queries using the same SQL (should use prepared
        // statement cache)
        let mut handles = vec![];
        for _ in 0..10 {
            let conn_clone = Arc::clone(&conn);
            let handle = tokio::spawn(async move {
                let mut results = vec![];
                for i in 0..10 {
                    let row: (i32, String) =
                        query_as("SELECT id, data FROM test_concurrent_prepared WHERE id = ?")
                            .bind(i * 10)
                            .fetch_one(&*conn_clone)
                            .await
                            .unwrap();
                    results.push(row);
                }
                results
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = join_all(handles).await;

        // Verify all tasks completed successfully
        for result in results {
            assert_eq!(result?.len(), 10);
        }

        Ok(())
    }

    /// Test that concurrent reads and writes work together
    #[tokio::test]
    async fn test_concurrent_read_write_mix() -> anyhow::Result<()> {
        let conn = Arc::new(connection().await?);

        // Setup test table
        query(
            "CREATE TABLE test_concurrent_mix (id INTEGER PRIMARY KEY, counter INTEGER DEFAULT 0)",
        )
        .execute(&*conn)
        .await?;

        // Insert initial data
        for i in 0..10 {
            query("INSERT INTO test_concurrent_mix (id, counter) VALUES (?, 0)")
                .bind(i)
                .execute(&*conn)
                .await?;
        }

        // Run concurrent readers and writers
        let mut handles = vec![];

        // Start readers
        for _reader_id in 0..5 {
            let conn_clone = Arc::clone(&conn);
            let handle = tokio::spawn(async move {
                for _ in 0..20 {
                    let total: (i64,) = query_as("SELECT SUM(counter) FROM test_concurrent_mix")
                        .fetch_one(&*conn_clone)
                        .await
                        .unwrap();
                    // Just verify we can read the data
                    assert!(total.0 >= 0);
                }
            });
            handles.push(handle);
        }

        // Start writers
        for writer_id in 0..3 {
            let conn_clone = Arc::clone(&conn);
            let handle = tokio::spawn(async move {
                for _ in 0..10 {
                    let id = writer_id % 10;
                    query("UPDATE test_concurrent_mix SET counter = counter + 1 WHERE id = ?")
                        .bind(id)
                        .execute(&*conn_clone)
                        .await
                        .unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for result in join_all(handles).await {
            result?;
        }

        // Verify final state
        let final_total: (i64,) = query_as("SELECT SUM(counter) FROM test_concurrent_mix")
            .fetch_one(&*conn)
            .await?;
        assert_eq!(final_total.0, 30); // 3 writers * 10 updates each

        Ok(())
    }

    /// Test that arguments are properly cloned and not consumed
    #[tokio::test]
    async fn test_arguments_not_consumed() -> anyhow::Result<()> {
        let conn = Arc::new(connection().await?);

        // Setup test table
        query("CREATE TABLE test_args (id INTEGER, value TEXT)")
            .execute(&*conn)
            .await?;

        // Bind once, then clone the query for concurrent use.
        let test_query = query("SELECT ?1 as id, ?2 as value")
            .bind(42)
            .bind("test_value");

        let mut handles = vec![];
        for _ in 0..5 {
            let conn_clone = Arc::clone(&conn);
            let query_clone = test_query.clone();
            let handle = tokio::spawn(async move {
                let row = query_clone.fetch_one(&*conn_clone).await.unwrap();
                let id: i32 = row.get_value("id").unwrap();
                let value: String = row.get_value("value").unwrap();
                (id, value)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = join_all(handles).await;

        // Verify all tasks completed successfully with correct results
        for result in results {
            let (id, value) = result?;
            assert_eq!(id, 42);
            assert_eq!(value, "test_value");
        }

        Ok(())
    }

    /// Test concurrent access to statement cache
    #[tokio::test]
    async fn test_concurrent_statement_cache() -> anyhow::Result<()> {
        let conn = Arc::new(connection().await?);

        // Create different SQL statements that should be cached
        let statements = [
            "SELECT 1 as num",
            "SELECT 2 as num",
            "SELECT 3 as num",
            "SELECT 4 as num",
            "SELECT 5 as num",
        ];

        // Run concurrent queries using different statements
        let mut handles = vec![];
        for i in 0..20 {
            let conn_clone = Arc::clone(&conn);
            let stmt = statements[i % statements.len()].to_string();
            let expected = (i % statements.len()) as i32 + 1;

            let handle = tokio::spawn(async move {
                let result: (i32,) = query_as(&stmt).fetch_one(&*conn_clone).await.unwrap();
                (result.0, expected)
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = join_all(handles).await;

        // Verify all tasks completed with correct results
        for result in results {
            let (actual, expected) = result?;
            assert_eq!(actual, expected);
        }

        Ok(())
    }

    /// Test that connections can be shared across threads safely
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_connection_thread_safety() -> anyhow::Result<()> {
        let conn = Arc::new(connection().await?);

        // Setup test table
        query("CREATE TABLE test_thread_safety (id INTEGER, thread_name TEXT)")
            .execute(&*conn)
            .await?;

        // Spawn tasks across the runtime worker threads
        let mut handles = vec![];
        for i in 0..4 {
            let conn_clone = Arc::clone(&conn);
            let handle = tokio::spawn(async move {
                let thread_name = format!("thread_{i}");

                // Insert data
                query("INSERT INTO test_thread_safety (id, thread_name) VALUES (?, ?)")
                    .bind(i)
                    .bind(&thread_name)
                    .execute(&*conn_clone)
                    .await?;

                // Read it back
                let result: (i32, String) =
                    query_as("SELECT id, thread_name FROM test_thread_safety WHERE id = ?")
                        .bind(i)
                        .fetch_one(&*conn_clone)
                        .await?;

                anyhow::Ok((result.0, result.1))
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = join_all(handles).await;

        // Verify all tasks completed successfully
        for (i, result) in results.into_iter().enumerate() {
            let (id, thread_name) = result??;
            assert_eq!(id, i as i32);
            assert_eq!(thread_name, format!("thread_{i}"));
        }

        Ok(())
    }
}
