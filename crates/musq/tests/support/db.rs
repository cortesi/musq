use std::{path::Path, time::Duration};

use musq::{Connection, JournalMode, Musq, Pool, query, query_scalar};

use crate::support::connection;

/// SQL schema used by integration tests.
const TEST_SCHEMA: &str = include_str!("setup.sql");

/// Populated table used by snapshot and copy tests.
const POPULATED_SCHEMA: &str = "
    CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL);
    INSERT INTO items(name) VALUES ('one'), ('two');
";

/// Return a connection to a database pre-configured with our test schema.
pub async fn tdb() -> anyhow::Result<Connection> {
    let conn = connection().await?;
    query(TEST_SCHEMA).execute(&conn).await?;
    Ok(conn)
}

/// Return a connection with a populated `items` table.
pub async fn populated_connection() -> anyhow::Result<Connection> {
    let conn = connection().await?;
    query(POPULATED_SCHEMA).execute(&conn).await?;
    Ok(conn)
}

/// Return a file-backed pool with a populated `items` table.
pub async fn populated_pool(path: &Path) -> anyhow::Result<Pool> {
    let pool = Musq::new().create_if_missing(true).open(path).await?;
    query(POPULATED_SCHEMA).execute(&pool).await?;
    Ok(pool)
}

/// Return a WAL pool with a populated `t(id, v)` table.
pub async fn wal_pool(dir: &Path) -> anyhow::Result<Pool> {
    let path = dir.join("tx.db");
    let pool = Musq::new()
        .create_if_missing(true)
        .journal_mode(JournalMode::Wal)
        .busy_timeout(Duration::from_millis(50))
        .max_connections(2)
        .open(&path)
        .await?;
    query("CREATE TABLE t (id INTEGER PRIMARY KEY, v INTEGER NOT NULL)")
        .execute(&pool)
        .await?;
    query("INSERT INTO t (id, v) VALUES (1, 0)")
        .execute(&pool)
        .await?;
    let mode: String = query_scalar("PRAGMA journal_mode").fetch_one(&pool).await?;
    assert_eq!(mode.to_ascii_lowercase(), "wal");
    Ok(pool)
}
