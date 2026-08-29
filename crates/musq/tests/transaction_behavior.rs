//! Transaction start modes, savepoints, and lock behavior.

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use musq::{
        JournalMode, Musq, TransactionBehavior, TxnState,
        error::{ExtendedErrCode, PrimaryErrCode},
        query, query_scalar,
    };
    use tempdir::TempDir;

    async fn wal_pool(dir: &TempDir) -> anyhow::Result<musq::Pool> {
        let path = dir.path().join("tx.db");
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

    #[tokio::test]
    async fn transaction_state_complements_autocommit() -> anyhow::Result<()> {
        let mut conn = musq::Connection::connect_with(&Musq::new()).await?;
        assert_eq!(conn.transaction_state().await?, TxnState::None);
        assert!(conn.is_autocommit().await?);

        let tx = conn.begin().await?;
        assert_eq!(tx.transaction_state().await?, TxnState::Write);
        assert!(!tx.is_autocommit().await?);
        tx.commit().await?;
        assert_eq!(conn.transaction_state().await?, TxnState::None);
        assert!(conn.is_autocommit().await?);

        query("CREATE TABLE t (x INTEGER)").execute(&conn).await?;
        let tx = conn.begin_with(TransactionBehavior::Deferred).await?;
        assert!(!tx.is_autocommit().await?);
        assert_eq!(tx.transaction_state().await?, TxnState::None);
        query("SELECT x FROM t").execute(&tx).await?;
        assert_eq!(tx.transaction_state().await?, TxnState::Read);
        query("INSERT INTO t (x) VALUES (1)").execute(&tx).await?;
        assert_eq!(tx.transaction_state().await?, TxnState::Write);
        tx.rollback().await?;
        assert_eq!(conn.transaction_state().await?, TxnState::None);
        assert!(conn.is_autocommit().await?);
        Ok(())
    }

    #[tokio::test]
    async fn autocommit_after_top_level_commit_and_rollback() -> anyhow::Result<()> {
        for behavior in [
            TransactionBehavior::Deferred,
            TransactionBehavior::Immediate,
            TransactionBehavior::Exclusive,
        ] {
            let mut conn =
                musq::Connection::connect_with(&Musq::new().default_transaction_behavior(behavior))
                    .await?;
            assert!(conn.is_autocommit().await?);

            let tx = conn.begin().await?;
            assert!(!tx.is_autocommit().await?);
            tx.commit().await?;
            assert!(conn.is_autocommit().await?);

            let tx = conn.begin_with(behavior).await?;
            assert!(!tx.is_autocommit().await?);
            tx.rollback().await?;
            assert!(conn.is_autocommit().await?);
        }
        Ok(())
    }

    #[tokio::test]
    async fn nested_savepoint_commit_and_rollback_leave_outer_open() -> anyhow::Result<()> {
        let mut conn = musq::Connection::connect_with(&Musq::new()).await?;
        query("CREATE TABLE t (id INTEGER PRIMARY KEY, v INTEGER NOT NULL)")
            .execute(&conn)
            .await?;

        let mut tx0 = conn.begin().await?;
        query("INSERT INTO t (id, v) VALUES (1, 0)")
            .execute(&tx0)
            .await?;
        assert!(!tx0.is_autocommit().await?);

        let tx1 = tx0.begin().await?;
        query("INSERT INTO t (id, v) VALUES (2, 0)")
            .execute(&tx1)
            .await?;
        assert!(!tx1.is_autocommit().await?);
        tx1.commit().await?;
        assert!(!tx0.is_autocommit().await?);

        let tx2 = tx0.begin().await?;
        query("INSERT INTO t (id, v) VALUES (3, 0)")
            .execute(&tx2)
            .await?;
        tx2.rollback().await?;
        assert!(!tx0.is_autocommit().await?);

        tx0.commit().await?;
        assert!(conn.is_autocommit().await?);

        let ids: Vec<(i64,)> = musq::query_as("SELECT id FROM t ORDER BY id")
            .fetch_all(&conn)
            .await?;
        assert_eq!(ids, vec![(1,), (2,)]);
        Ok(())
    }

    #[tokio::test]
    async fn immediate_blocks_a_second_writer_with_busy() -> anyhow::Result<()> {
        let dir = TempDir::new("musq-tx-immediate")?;
        let pool = wal_pool(&dir).await?;

        let tx = pool.begin_with(TransactionBehavior::Immediate).await?;
        let error = query("UPDATE t SET v = 1")
            .execute(&pool)
            .await
            .unwrap_err();
        let sqlite = error.as_sqlite().unwrap_or_else(|| {
            panic!("expected SQLite busy error, got {error:?}");
        });
        assert_eq!(sqlite.primary, PrimaryErrCode::Busy);
        assert_ne!(sqlite.extended, Some(ExtendedErrCode::BusySnapshot));

        tx.rollback().await?;
        Ok(())
    }

    #[tokio::test]
    async fn deferred_read_then_write_hits_busy_snapshot() -> anyhow::Result<()> {
        let dir = TempDir::new("musq-tx-deferred")?;
        let pool = wal_pool(&dir).await?;

        let tx = pool.begin_with(TransactionBehavior::Deferred).await?;
        let _: i64 = query_scalar("SELECT v FROM t WHERE id = 1")
            .fetch_one(&tx)
            .await?;

        query("UPDATE t SET v = 1").execute(&pool).await?;

        let error = query("UPDATE t SET v = 2").execute(&tx).await.unwrap_err();
        let sqlite = error.as_sqlite().expect("SQLite snapshot error");
        assert_eq!(sqlite.primary, PrimaryErrCode::Busy);
        assert_eq!(sqlite.extended, Some(ExtendedErrCode::BusySnapshot));

        tx.rollback().await?;
        Ok(())
    }
}
