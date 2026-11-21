//! Integration tests for musq.

#[cfg(test)]
mod tests {
    use std::panic;

    use musq::Musq;
    use tokio::runtime::Runtime;

    #[test]
    fn pool_connection_drop_without_runtime_does_not_panic() {
        let res = panic::catch_unwind(|| {
            let rt = Runtime::new().expect("tokio runtime");
            let (pool, conn) = rt.block_on(async {
                let pool = Musq::new().open_in_memory().await.expect("pool");
                let conn = pool.acquire().await.expect("connection");
                (pool, conn)
            });

            drop(rt);
            drop(conn);
            drop(pool);
        });

        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn dropping_pool_connection_releases_synchronously() -> anyhow::Result<()> {
        let pool = Musq::new().max_connections(1).open_in_memory().await?;
        let conn = pool.acquire().await?;
        assert_eq!(pool.num_idle(), 0);
        assert!(pool.try_acquire().is_none());

        drop(conn);
        assert_eq!(pool.num_idle(), 1);
        assert!(pool.try_acquire().is_some());
        Ok(())
    }
}
