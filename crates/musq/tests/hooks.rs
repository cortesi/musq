//! Per-connection update, commit, and rollback hooks.

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use musq::{Connection, Musq, UpdateEvent, UpdateOp, query, query_scalar};
    use tokio::{sync::mpsc, time::timeout};

    #[tokio::test]
    async fn update_hook_sees_insert() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;
        query("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)")
            .execute(&conn)
            .await?;
        let (tx, mut rx) = mpsc::unbounded_channel();
        conn.on_update(move |event| {
            tx.send(event).ok();
        })
        .await?;
        query("INSERT INTO t (id, name) VALUES (1, 'one')")
            .execute(&conn)
            .await?;
        let event = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("update event")
            .expect("channel open");
        assert_eq!(
            event,
            UpdateEvent {
                op: UpdateOp::Insert,
                database: "main".into(),
                table: "t".into(),
                rowid: 1,
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn commit_and_rollback_hooks_fire() -> anyhow::Result<()> {
        let mut conn = Connection::connect_with(&Musq::new()).await?;
        query("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .execute(&conn)
            .await?;
        let (commit_tx, mut commit_rx) = mpsc::unbounded_channel();
        let (rollback_tx, mut rollback_rx) = mpsc::unbounded_channel();
        conn.on_commit(move || {
            commit_tx.send(()).ok();
        })
        .await?;
        conn.on_rollback(move || {
            rollback_tx.send(()).ok();
        })
        .await?;

        let tx = conn.begin().await?;
        query("INSERT INTO t (id) VALUES (1)").execute(&tx).await?;
        tx.commit().await?;
        timeout(Duration::from_secs(1), commit_rx.recv())
            .await
            .expect("commit event")
            .expect("channel open");

        let tx = conn.begin().await?;
        query("INSERT INTO t (id) VALUES (2)").execute(&tx).await?;
        tx.rollback().await?;
        timeout(Duration::from_secs(1), rollback_rx.recv())
            .await
            .expect("rollback event")
            .expect("channel open");

        let count: i64 = query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(&conn)
            .await?;
        assert_eq!(count, 1);
        Ok(())
    }

    #[tokio::test]
    async fn hook_panic_is_caught() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;
        query("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .execute(&conn)
            .await?;
        conn.on_update(|_| panic!("hook")).await?;
        query("INSERT INTO t (id) VALUES (1)")
            .execute(&conn)
            .await?;
        let one: i64 = query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(&conn)
            .await?;
        assert_eq!(one, 1);
        Ok(())
    }
}
