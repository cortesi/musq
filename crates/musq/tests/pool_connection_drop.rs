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
}
