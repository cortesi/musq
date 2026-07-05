//! Integration tests for modern bundled SQLite SQL features.

mod support;

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use musq::{Result as MusqResult, SqliteError, error::ExtendedErrCode, query, query_scalar};

    use crate::support::connection;

    #[tokio::test]
    async fn alter_table_constraint_changes_work() -> anyhow::Result<()> {
        let conn = connection().await?;

        query(
            "CREATE TABLE modern_users( \
                 id INTEGER PRIMARY KEY, \
                 name TEXT, \
                 age INTEGER \
             )",
        )
        .execute(&conn)
        .await?;
        query("INSERT INTO modern_users(id, name, age) VALUES (1, 'alice', 41)")
            .execute(&conn)
            .await?;

        query("ALTER TABLE modern_users ALTER COLUMN name SET NOT NULL")
            .execute(&conn)
            .await?;
        query("ALTER TABLE modern_users ALTER COLUMN name SET NOT NULL")
            .execute(&conn)
            .await?;

        let not_null_error = sqlite_error(
            query("INSERT INTO modern_users(id, name, age) VALUES (2, NULL, 10)")
                .execute(&conn)
                .await,
        );
        assert_eq!(not_null_error.extended, ExtendedErrCode::ConstraintNotNull);

        query("ALTER TABLE modern_users ALTER COLUMN name DROP NOT NULL")
            .execute(&conn)
            .await?;
        query("INSERT INTO modern_users(id, name, age) VALUES (2, NULL, 10)")
            .execute(&conn)
            .await?;
        let null_name_count: i64 =
            query_scalar("SELECT COUNT(*) FROM modern_users WHERE name IS NULL")
                .fetch_one(&conn)
                .await?;
        assert_eq!(null_name_count, 1);

        query("ALTER TABLE modern_users ADD CONSTRAINT age_nonnegative CHECK (age >= 0)")
            .execute(&conn)
            .await?;

        let check_error = sqlite_error(
            query("INSERT INTO modern_users(id, name, age) VALUES (3, 'bob', -1)")
                .execute(&conn)
                .await,
        );
        assert_eq!(check_error.extended, ExtendedErrCode::ConstraintCheck);
        assert!(check_error.message.contains("age_nonnegative"));

        query("ALTER TABLE modern_users DROP CONSTRAINT age_nonnegative")
            .execute(&conn)
            .await?;
        query("INSERT INTO modern_users(id, name, age) VALUES (3, 'bob', -1)")
            .execute(&conn)
            .await?;
        let negative_age_count: i64 =
            query_scalar("SELECT COUNT(*) FROM modern_users WHERE age < 0")
                .fetch_one(&conn)
                .await?;
        assert_eq!(negative_age_count, 1);

        Ok(())
    }

    #[tokio::test]
    async fn reindex_expressions_accepts_expression_indexes() -> anyhow::Result<()> {
        let conn = connection().await?;

        query("CREATE TABLE expression_items(id INTEGER PRIMARY KEY, name TEXT)")
            .execute(&conn)
            .await?;
        query("CREATE INDEX expression_items_lower_name ON expression_items(lower(name))")
            .execute(&conn)
            .await?;
        query("INSERT INTO expression_items(id, name) VALUES (1, 'Alice')")
            .execute(&conn)
            .await?;

        query("REINDEX EXPRESSIONS").execute(&conn).await?;

        let id: i64 =
            query_scalar("SELECT id FROM expression_items WHERE lower(name) = lower('alice')")
                .fetch_one(&conn)
                .await?;
        assert_eq!(id, 1);

        Ok(())
    }

    #[tokio::test]
    async fn json_array_insert_functions_round_trip() -> anyhow::Result<()> {
        let conn = connection().await?;

        let json_text: String = query_scalar("SELECT json_array_insert(?, ?, ?)")
            .bind("[1,2,3]")
            .bind("$[1]")
            .bind("new")
            .fetch_one(&conn)
            .await?;
        assert_eq!(json_text, r#"[1,"new",2,3]"#);

        let jsonb_text: String = query_scalar("SELECT json(jsonb_array_insert(jsonb(?), ?, ?))")
            .bind("[1,2,3]")
            .bind("$[1]")
            .bind("new")
            .fetch_one(&conn)
            .await?;
        assert_eq!(jsonb_text, r#"[1,"new",2,3]"#);

        Ok(())
    }

    fn sqlite_error<T>(result: MusqResult<T>) -> SqliteError
    where
        T: Debug,
    {
        result.unwrap_err().into_sqlite_error().unwrap()
    }
}
