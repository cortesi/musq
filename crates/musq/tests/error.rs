//! Integration tests for musq.

mod support;
#[path = "support/db.rs"]
mod support_db;

#[cfg(test)]
mod tests {
    use musq::{
        Result, SqliteError,
        error::{Error, ExtendedErrCode, PrimaryErrCode},
        query,
    };

    use crate::support_db::tdb;

    #[tokio::test]
    async fn it_fails_with_unique_violation() -> anyhow::Result<()> {
        let mut conn = tdb().await?;
        let tx = conn.begin().await?;

        let res: Result<_> = query("INSERT INTO tweet VALUES (1, 'Foo', true, 1);")
            .execute(&tx)
            .await;
        let err = res.unwrap_err();

        assert!(err.is_unique_violation());
        assert_eq!(
            err.sqlite_codes(),
            Some((
                PrimaryErrCode::Constraint,
                Some(ExtendedErrCode::ConstraintPrimaryKey)
            ))
        );

        let err = err.as_sqlite().unwrap();
        assert!(err.message.contains("constraint"));
        Ok(())
    }

    #[tokio::test]
    async fn it_classifies_unique_constraint_violations() -> anyhow::Result<()> {
        let pool = musq::Musq::new().open_in_memory().await?;
        query("CREATE TABLE contacts(id INTEGER PRIMARY KEY, email TEXT UNIQUE)")
            .execute(&pool)
            .await?;
        query("INSERT INTO contacts VALUES (1, 'same@example.com')")
            .execute(&pool)
            .await?;

        let error = query("INSERT INTO contacts VALUES (2, 'same@example.com')")
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(error.is_unique_violation());
        assert_eq!(
            error.sqlite_codes(),
            Some((
                PrimaryErrCode::Constraint,
                Some(ExtendedErrCode::ConstraintUnique)
            ))
        );
        Ok(())
    }

    #[test]
    fn it_classifies_busy_codes_and_non_sqlite_errors() {
        for extended in [
            ExtendedErrCode::BusyRecovery,
            ExtendedErrCode::BusySnapshot,
            ExtendedErrCode::BusyTimeout,
        ] {
            let error = Error::Sqlite(SqliteError {
                primary: PrimaryErrCode::Busy,
                extended: Some(extended),
                message: "busy".into(),
                offset: None,
            });
            assert!(error.is_busy());
            assert!(!error.is_unique_violation());
        }

        let other_sqlite = Error::Sqlite(SqliteError {
            primary: PrimaryErrCode::Error,
            extended: None,
            message: "other".into(),
            offset: None,
        });
        assert!(!other_sqlite.is_busy());
        assert!(!other_sqlite.is_unique_violation());

        let non_sqlite = Error::Protocol("not SQLite".into());
        assert_eq!(non_sqlite.sqlite_codes(), None);
        assert!(!non_sqlite.is_busy());
        assert!(!non_sqlite.is_unique_violation());
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

        let err = err.as_sqlite().unwrap();

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

        let err = err.as_sqlite().unwrap();

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

        let err = err.as_sqlite().unwrap();

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
        assert!(err.as_sqlite().is_some());

        Ok(())
    }

    #[tokio::test]
    async fn syntax_error_reports_offset_and_plain_code() -> anyhow::Result<()> {
        let pool = musq::Musq::new().open_in_memory().await?;
        let error = query("SELECT * FORM t").execute(&pool).await.unwrap_err();
        let sqlite = error.as_sqlite().expect("SQLite syntax error");
        assert_eq!(sqlite.primary, PrimaryErrCode::Error);
        assert_eq!(sqlite.extended, None);
        assert_eq!(sqlite.offset, Some(9));
        assert_eq!(
            error.to_string(),
            r#"SQLITE_ERROR at byte 9: near "FORM": syntax error"#
        );
        let _ = pool.close().await;
        Ok(())
    }
}
