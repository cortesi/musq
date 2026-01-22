//! Integration tests for musq.

mod support;
#[path = "support/db.rs"]
mod support_db;

#[cfg(test)]
mod tests {
    use musq::{Result, query};

    use crate::support_db::tdb;

    #[tokio::test]
    async fn it_fails_with_unique_violation() -> anyhow::Result<()> {
        let mut conn = tdb().await?;
        let tx = conn.begin().await?;

        let res: Result<_> = query("INSERT INTO tweet VALUES (1, 'Foo', true, 1);")
            .execute(&tx)
            .await;
        let err = res.unwrap_err();

        let err = err.into_sqlite_error().unwrap();
        assert!(err.message.contains("constraint"));
        Ok(())
    }

    #[tokio::test]
    async fn it_fails_with_foreign_key_violation() -> anyhow::Result<()> {
        let mut conn = tdb().await?;
        let tx = conn.begin().await?;

        let res: Result<_> =
            query("INSERT INTO tweet_reply (id, tweet_id, text) VALUES (2, 2, 'Reply!');")
                .execute(&tx)
                .await;
        let err = res.unwrap_err();

        let err = err.into_sqlite_error().unwrap();

        assert!(err.message.contains("constraint"));

        Ok(())
    }

    #[tokio::test]
    async fn it_fails_with_not_null_violation() -> anyhow::Result<()> {
        let mut conn = tdb().await?;
        let tx = conn.begin().await?;

        let res: Result<_> = query("INSERT INTO tweet (text) VALUES (null);")
            .execute(&tx)
            .await;
        let err = res.unwrap_err();

        let err = err.into_sqlite_error().unwrap();

        assert!(err.message.contains("constraint"));

        Ok(())
    }

    #[tokio::test]
    async fn it_fails_with_check_violation() -> anyhow::Result<()> {
        let mut conn = tdb().await?;
        let tx = conn.begin().await?;

        let res: Result<_> = query("INSERT INTO products VALUES (1, 'Product 1', 0);")
            .execute(&tx)
            .await;
        let err = res.unwrap_err();

        let err = err.into_sqlite_error().unwrap();

        assert!(err.message.contains("constraint"));

        Ok(())
    }

    #[tokio::test]
    async fn it_fails_to_open() -> anyhow::Result<()> {
        use musq::{Connection, Musq};
        use tempdir::TempDir;

        let dir = TempDir::new("musq-open-fail")?;
        let path = dir.path().join("nonexistent.db");

        let options = Musq::new().filename(&path);
        let res = Connection::connect_with(&options).await;

        let err = res.unwrap_err();
        println!("error: {err:?}");
        assert!(err.into_sqlite_error().is_some());

        Ok(())
    }
}
