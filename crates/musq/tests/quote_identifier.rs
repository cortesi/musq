//! Integration tests for musq.

mod support;

#[cfg(test)]
mod tests {
    use musq::{query, query_scalar, quote_identifier};

    use crate::support::connection;

    #[tokio::test]
    async fn quote_identifier_escapes_double_quotes() -> anyhow::Result<()> {
        assert_eq!(quote_identifier("user\"name"), "\"user\"\"name\"");

        let conn = connection().await?;
        let ident = "user\"name";
        query(&format!(
            "CREATE TABLE {} (id INTEGER)",
            quote_identifier(ident)
        ))
        .execute(&conn)
        .await?;
        query(&format!(
            "INSERT INTO {} (id) VALUES (1)",
            quote_identifier(ident)
        ))
        .execute(&conn)
        .await?;
        let val: i32 = query_scalar(&format!("SELECT id FROM {}", quote_identifier(ident)))
            .fetch_one(&conn)
            .await?;
        assert_eq!(val, 1);
        Ok(())
    }
}
