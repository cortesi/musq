use musq::{Connection, Executor, Musq, Pool, query, query_as};
use std::sync::Arc;

/// This example demonstrates that query.execute() now accepts
/// Pool, Connection, PoolConnection, and Transaction interchangeably
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a pool and standalone connection
    let pool = Arc::new(Musq::new().open_in_memory().await?);
    let standalone_conn = Connection::connect_with(&Musq::new()).await?;

    // Setup schema on both
    setup_schema(&*pool).await?;
    setup_schema(&standalone_conn).await?;

    println!("✅ All executor types work interchangeably with query.execute()!");

    // Test with Pool directly
    insert_user(&*pool, 1, "Alice").await?;
    println!("📊 Inserted user via Pool");

    // Test with PoolConnection
    let pool_conn = pool.acquire().await?;
    insert_user(&pool_conn, 2, "Bob").await?;
    println!("📊 Inserted user via PoolConnection");

    // Test with standalone Connection
    insert_user(&standalone_conn, 1, "Charlie").await?;
    println!("📊 Inserted user via standalone Connection");

    // Test with Transaction
    let mut tx = pool.begin().await?;
    insert_user(&tx, 3, "Diana").await?;
    tx.commit().await?;
    println!("📊 Inserted user via Transaction");

    // Query data back using different executor types
    let pool_users = get_users(&*pool).await?;
    println!("👥 Pool users: {:?}", pool_users);

    let standalone_users = get_users(&standalone_conn).await?;
    println!("👥 Standalone users: {:?}", standalone_users);

    // Demonstrate concurrent usage with Pool
    println!("🚀 Testing concurrent pool usage...");
    test_concurrent_pool_usage(&pool).await?;

    Ok(())
}

/// Generic function that works with any Executor
async fn setup_schema<E: for<'a> Executor<'a>>(executor: &E) -> musq::Result<()> {
    query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(executor)
        .await?;
    Ok(())
}

/// Generic function that works with any Executor
async fn insert_user<E: for<'a> Executor<'a>>(
    executor: &E,
    id: i32,
    name: &str,
) -> musq::Result<()> {
    query("INSERT INTO users (id, name) VALUES (?, ?)")
        .bind(id)
        .bind(name)
        .execute(executor)
        .await?;
    Ok(())
}

/// Generic function that works with any Executor
async fn get_users<E: for<'a> Executor<'a> + Send + Sync>(
    executor: &E,
) -> musq::Result<Vec<(i32, String)>> {
    query_as("SELECT id, name FROM users ORDER BY id")
        .fetch_all(executor)
        .await
}

/// Test concurrent usage of Pool as Executor
async fn test_concurrent_pool_usage(pool: &Arc<Pool>) -> musq::Result<()> {
    let mut handles = vec![];

    for i in 10..15 {
        let pool_clone = Arc::clone(pool);
        let handle = tokio::spawn(async move {
            let name = format!("User{}", i);
            insert_user(&*pool_clone, i, &name).await
        });
        handles.push(handle);
    }

    // Wait for all insertions to complete
    for handle in handles {
        handle.await.unwrap()?;
    }

    // Count total users
    let count: (i64,) = query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&**pool)
        .await?;

    println!("🎯 Total users after concurrent operations: {}", count.0);
    Ok(())
}
