use musq::{Musq, Pool, PoolConnection, query};

// Helper function to create a test database
async fn setup_test_db() -> musq::Result<PoolConnection> {
    let pool = Musq::new().open_in_memory().await?;
    let mut conn = pool.acquire().await?;

    // Create a test table
    query("CREATE TABLE test_table (id INTEGER PRIMARY KEY, value TEXT)")
        .execute(&conn)
        .await?;

    // Insert test data
    query("INSERT INTO test_table (value) VALUES (?)")
        .bind("test_value_1")
        .execute(&conn)
        .await?;

    query("INSERT INTO test_table (value) VALUES (?)")
        .bind("test_value_2")
        .execute(&conn)
        .await?;

    Ok(conn)
}

// Helper function to create a test pool
async fn setup_test_pool() -> musq::Result<Pool> {
    let pool = Musq::new().open_in_memory().await?;

    let mut conn = pool.acquire().await?;

    // Create a test table
    query("CREATE TABLE test_table (id INTEGER PRIMARY KEY, value TEXT)")
        .execute(&conn)
        .await?;

    // Insert test data
    query("INSERT INTO test_table (value) VALUES (?)")
        .bind("test_value_1")
        .execute(&conn)
        .await?;

    query("INSERT INTO test_table (value) VALUES (?)")
        .bind("test_value_2")
        .execute(&conn)
        .await?;

    Ok(pool)
}

mod connection_executor {
    use super::*;
    use musq::Row;

    #[tokio::test]
    async fn test_execute_with_connection() -> musq::Result<()> {
        let mut conn = setup_test_db().await?;

        let result = query("INSERT INTO test_table (value) VALUES (?)")
            .bind("new_value")
            .execute(&conn)
            .await?;

        assert_eq!(result.rows_affected(), 1);
        assert!(result.last_insert_rowid() > 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_one_with_connection() -> musq::Result<()> {
        let mut conn = setup_test_db().await?;

        let row: Row = query("SELECT value FROM test_table WHERE id = ?")
            .bind(1)
            .fetch_one(&conn)
            .await?;

        let value: String = row.get_value("value")?;
        assert_eq!(value, "test_value_1");
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_optional_with_connection() -> musq::Result<()> {
        let mut conn = setup_test_db().await?;

        let row: Option<Row> = query("SELECT value FROM test_table WHERE id = ?")
            .bind(999)
            .fetch_optional(&conn)
            .await?;

        assert!(row.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_all_with_connection() -> musq::Result<()> {
        let mut conn = setup_test_db().await?;

        let rows: Vec<Row> = query("SELECT value FROM test_table ORDER BY id")
            .fetch_all(&conn)
            .await?;

        assert_eq!(rows.len(), 2);
        let value1: String = rows[0].get_value("value")?;
        let value2: String = rows[1].get_value("value")?;
        assert_eq!(value1, "test_value_1");
        assert_eq!(value2, "test_value_2");
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_stream_with_connection() -> musq::Result<()> {
        use futures_util::TryStreamExt;

        let mut conn = setup_test_db().await?;

        let rows: Vec<Row> = query("SELECT value FROM test_table ORDER BY id")
            .fetch(&conn)
            .try_collect()
            .await?;

        assert_eq!(rows.len(), 2);
        Ok(())
    }
}

mod transaction_executor {
    use super::*;
    use musq::Row;

    #[tokio::test]
    async fn test_execute_with_transaction_commit() -> musq::Result<()> {
        let mut conn = setup_test_db().await?;

        {
            let mut tx = conn.begin().await?;

            let result = query("INSERT INTO test_table (value) VALUES (?)")
                .bind("tx_value")
                .execute(&tx)
                .await?;

            assert_eq!(result.rows_affected(), 1);
            tx.commit().await?;
        }

        // Verify the value was committed
        let row: Row = query("SELECT value FROM test_table WHERE value = ?")
            .bind("tx_value")
            .fetch_one(&conn)
            .await?;

        let value: String = row.get_value("value")?;
        assert_eq!(value, "tx_value");
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_with_transaction_rollback() -> musq::Result<()> {
        let mut conn = setup_test_db().await?;

        {
            let mut tx = conn.begin().await?;

            query("INSERT INTO test_table (value) VALUES (?)")
                .bind("tx_rollback_value")
                .execute(&tx)
                .await?;

            tx.rollback().await?;
        }

        // Verify the value was not committed
        let row: Option<Row> = query("SELECT value FROM test_table WHERE value = ?")
            .bind("tx_rollback_value")
            .fetch_optional(&conn)
            .await?;

        assert!(row.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_one_with_transaction() -> musq::Result<()> {
        let mut conn = setup_test_db().await?;

        let mut tx = conn.begin().await?;

        let row: Row = query("SELECT value FROM test_table WHERE id = ?")
            .bind(1)
            .fetch_one(&tx)
            .await?;

        let value: String = row.get_value("value")?;
        assert_eq!(value, "test_value_1");

        tx.commit().await?;
        Ok(())
    }
}

mod pool_execution {
    use super::*;
    use musq::Row;

    #[tokio::test]
    async fn test_execute_with_pool() -> musq::Result<()> {
        let pool = setup_test_pool().await?;

        let result = pool
            .execute(query("INSERT INTO test_table (value) VALUES (?)").bind("pool_value"))
            .await?;

        assert_eq!(result.rows_affected(), 1);
        assert!(result.last_insert_rowid() > 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_one_with_pool() -> musq::Result<()> {
        let pool = setup_test_pool().await?;

        let row: Row = pool
            .fetch_one(query("SELECT value FROM test_table WHERE id = ?").bind(1))
            .await?;

        let value: String = row.get_value("value")?;
        assert_eq!(value, "test_value_1");
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_optional_with_pool() -> musq::Result<()> {
        let pool = setup_test_pool().await?;

        let row: Option<Row> = pool
            .fetch_optional(query("SELECT value FROM test_table WHERE id = ?").bind(999))
            .await?;

        assert!(row.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_all_with_pool() -> musq::Result<()> {
        let pool = setup_test_pool().await?;

        let rows: Vec<Row> = pool
            .fetch_all(query("SELECT value FROM test_table ORDER BY id"))
            .await?;

        assert_eq!(rows.len(), 2);
        let value1: String = rows[0].get_value("value")?;
        let value2: String = rows[1].get_value("value")?;
        assert_eq!(value1, "test_value_1");
        assert_eq!(value2, "test_value_2");
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_pool_execution() -> musq::Result<()> {
        let pool = setup_test_pool().await?;

        // Test concurrent access to the pool
        let futures = (0..5).map(|i| {
            let pool = pool.clone();
            let value = format!("concurrent_value_{}", i);
            async move {
                pool.execute(query("INSERT INTO test_table (value) VALUES (?)").bind(&value))
                    .await
            }
        });

        let results = futures_util::future::try_join_all(futures).await?;

        // All inserts should succeed
        for result in results {
            assert_eq!(result.rows_affected(), 1);
        }

        // Verify all values were inserted
        let count: Row = pool
            .fetch_one(query("SELECT COUNT(*) as count FROM test_table"))
            .await?;

        let count_value: i64 = count.get_value("count")?;
        assert_eq!(count_value, 7); // 2 initial + 5 concurrent

        Ok(())
    }
}

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn test_execute_invalid_sql_with_connection() -> musq::Result<()> {
        let mut conn = setup_test_db().await?;

        let result = query("INVALID SQL STATEMENT").execute(&conn).await;

        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_one_not_found_with_connection() -> musq::Result<()> {
        let mut conn = setup_test_db().await?;

        let result = query("SELECT value FROM test_table WHERE id = ?")
            .bind(999)
            .fetch_one(&conn)
            .await;

        assert!(result.is_err());
        if let Err(musq::Error::RowNotFound) = result {
            // Expected error
        } else {
            panic!("Expected RowNotFound error");
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_invalid_sql_with_pool() -> musq::Result<()> {
        let pool = setup_test_pool().await?;

        let result = pool.execute(query("INVALID SQL STATEMENT")).await;

        assert!(result.is_err());
        Ok(())
    }
}
