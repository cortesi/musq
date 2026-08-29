//! Query cancellation through `sqlite3_interrupt` and statement timeouts.

#[cfg(test)]
mod tests {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use musq::{Connection, Error, Musq, error::PrimaryErrCode, query};
    use tokio::{
        join,
        time::{error::Elapsed, sleep, timeout},
    };

    /// Recursive CTE with no bound. SQLite runs this until interrupt or timeout.
    const INFINITE_SELECT: &str =
        "WITH RECURSIVE t(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM t) SELECT x FROM t";

    /// Infinite insert used to interrupt a write inside a transaction.
    const INFINITE_INSERT: &str = "INSERT INTO t (x) \
     SELECT x FROM (WITH RECURSIVE t(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM t) SELECT x FROM t)";

    fn assert_interrupt(err: &Error) {
        assert_eq!(
            err.as_sqlite().map(|error| error.primary),
            Some(PrimaryErrCode::Interrupt),
            "expected SQLITE_INTERRUPT, got {err:?}"
        );
    }

    fn expect_interrupt<T>(timed: Result<Result<T, Error>, Elapsed>) -> Error {
        match timed {
            Err(_) => panic!("interrupt should complete before the test timeout"),
            Ok(Err(error)) => error,
            Ok(Ok(_)) => panic!("query should fail with SQLITE_INTERRUPT"),
        }
    }

    #[tokio::test]
    async fn interrupt_recursive_cte() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;
        let handle = conn.interrupt_handle();

        let fetch = query(INFINITE_SELECT).fetch_all(&conn);
        let interrupt = async {
            sleep(Duration::from_millis(20)).await;
            handle.interrupt();
        };

        let (result, _) = join!(timeout(Duration::from_secs(5), fetch), interrupt);
        let err = expect_interrupt(result);
        assert_interrupt(&err);

        let one: i32 = musq::query_scalar("SELECT 1").fetch_one(&conn).await?;
        assert_eq!(one, 1);
        Ok(())
    }

    #[tokio::test]
    async fn interrupt_write_aborts_commit() -> anyhow::Result<()> {
        let mut conn = Connection::connect_with(&Musq::new()).await?;
        query("CREATE TABLE t (x INTEGER)").execute(&conn).await?;

        let tx = conn.begin().await?;
        let handle = tx.interrupt_handle();

        let insert = query(INFINITE_INSERT).execute(&tx);
        let interrupt = async {
            sleep(Duration::from_millis(20)).await;
            handle.interrupt();
        };

        let (result, _) = join!(timeout(Duration::from_secs(5), insert), interrupt);
        let err = expect_interrupt(result);
        assert_interrupt(&err);

        let commit_err = tx.commit().await.unwrap_err();
        assert!(
            matches!(commit_err, Error::TransactionAborted),
            "expected TransactionAborted, got {commit_err:?}"
        );

        let count: i64 = musq::query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(&conn)
            .await?;
        assert_eq!(count, 0);

        query("INSERT INTO t (x) VALUES (1)").execute(&conn).await?;
        Ok(())
    }

    #[tokio::test]
    async fn interrupt_races_close() -> anyhow::Result<()> {
        for _ in 0..1000 {
            let conn = Connection::connect_with(&Musq::new()).await?;
            let handle = conn.interrupt_handle();
            let interruptor = thread::spawn(move || {
                for _ in 0..32 {
                    handle.interrupt();
                }
            });
            conn.close().await?;
            interruptor.join().expect("interrupt thread");
        }
        Ok(())
    }

    #[tokio::test]
    async fn statement_timeout_inside_transaction() -> anyhow::Result<()> {
        let mut conn =
            Connection::connect_with(&Musq::new().statement_timeout(Duration::from_millis(50)))
                .await?;
        query("CREATE TABLE t (x INTEGER)").execute(&conn).await?;

        let tx = conn.begin().await?;
        let started = Instant::now();
        let err = expect_interrupt(
            timeout(Duration::from_secs(5), query(INFINITE_INSERT).execute(&tx)).await,
        );
        assert_interrupt(&err);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timeout took {:?}",
            started.elapsed()
        );

        let commit_err = tx.commit().await.unwrap_err();
        assert!(
            matches!(commit_err, Error::TransactionAborted),
            "expected TransactionAborted, got {commit_err:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn pool_connection_interrupt() -> anyhow::Result<()> {
        let pool = Musq::new().max_connections(1).open_in_memory().await?;
        let conn = pool.acquire().await?;
        let handle = conn.interrupt_handle();

        let fetch = query(INFINITE_SELECT).fetch_all(&conn);
        let interrupt = async {
            sleep(Duration::from_millis(20)).await;
            conn.interrupt();
            handle.interrupt();
        };

        let (result, _) = join!(timeout(Duration::from_secs(5), fetch), interrupt);
        let err = expect_interrupt(result);
        assert_interrupt(&err);
        drop(conn);
        Ok(())
    }

    #[test]
    fn interrupt_handle_is_shareable() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<musq::InterruptHandle>();
    }
}
