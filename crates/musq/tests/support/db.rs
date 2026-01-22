use musq::{Connection, query as query_mod};

use crate::support::connection;

/// SQL schema used by integration tests.
const TEST_SCHEMA: &str = include_str!("setup.sql");

/// Return a connection to a database pre-configured with our test schema.
pub async fn tdb() -> anyhow::Result<Connection> {
    let conn = connection().await?;
    query_mod::query(TEST_SCHEMA).execute(&conn).await?;
    Ok(conn)
}
