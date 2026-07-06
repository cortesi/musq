//! Integration tests for musq.

#[cfg(test)]
mod tests {
    use musq::{Error, Musq, Pool, PoolStats};
    use tokio::{
        task::yield_now,
        time::{Duration, sleep, timeout},
    };

    async fn wait_for_stats(pool: &Pool, predicate: impl Fn(PoolStats) -> bool) -> PoolStats {
        timeout(Duration::from_secs(1), async {
            loop {
                let stats = pool.stats();
                if predicate(stats) {
                    return stats;
                }
                yield_now().await;
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("timed out waiting for pool stats")
    }

    #[tokio::test]
    async fn close_while_waiting_does_not_panic() -> anyhow::Result<()> {
        let pool = Musq::new().max_connections(1).open_in_memory().await?;

        // Hold the only connection so subsequent acquires must wait
        let conn = pool.acquire().await?;

        let pool_for_waiter = pool.clone();
        let waiter = tokio::spawn(async move { pool_for_waiter.acquire().await });

        // ensure the waiter is blocking on acquire
        sleep(Duration::from_millis(50)).await;

        let pool_for_close = pool.clone();
        let closer = tokio::spawn(async move {
            let _ = pool_for_close.close().await;
        });

        sleep(Duration::from_millis(50)).await;
        drop(conn); // release the connection so close can finish

        closer.await.expect("close task panicked");
        let res = waiter.await.expect("waiter task panicked");
        assert!(matches!(res, Err(Error::PoolClosed)));

        Ok(())
    }

    #[tokio::test]
    async fn close_waits_for_checked_out_connection_after_closing_idle() -> anyhow::Result<()> {
        let pool = Musq::new().max_connections(2).open_in_memory().await?;

        let idle = pool.acquire().await?;
        let held = pool.acquire().await?;
        drop(idle);

        wait_for_stats(&pool, |stats| stats.size == 2 && stats.num_idle == 1).await;

        let pool_for_close = pool.clone();
        let mut closer = tokio::spawn(async move {
            let _ = pool_for_close.close().await;
        });

        assert!(
            timeout(Duration::from_millis(100), &mut closer)
                .await
                .is_err(),
            "close returned while a connection was still checked out"
        );
        assert_eq!(
            wait_for_stats(&pool, |stats| stats.size == 1 && stats.num_idle == 0).await,
            PoolStats {
                size: 1,
                num_idle: 0,
                is_closed: true,
            }
        );
        assert!(pool.try_acquire().is_none());

        drop(held);
        timeout(Duration::from_secs(1), closer)
            .await
            .expect("close timed out")
            .expect("close task panicked");

        assert_eq!(
            pool.stats(),
            PoolStats {
                size: 0,
                num_idle: 0,
                is_closed: true,
            }
        );

        Ok(())
    }

    #[tokio::test]
    async fn close_drains_multiple_idle_connections_before_waiting() -> anyhow::Result<()> {
        let pool = Musq::new().max_connections(3).open_in_memory().await?;

        let idle_a = pool.acquire().await?;
        let idle_b = pool.acquire().await?;
        let held = pool.acquire().await?;
        drop(idle_a);
        drop(idle_b);

        wait_for_stats(&pool, |stats| stats.size == 3 && stats.num_idle == 2).await;

        let pool_for_close = pool.clone();
        let mut closer = tokio::spawn(async move {
            let _ = pool_for_close.close().await;
        });

        assert!(
            timeout(Duration::from_millis(100), &mut closer)
                .await
                .is_err(),
            "close returned before the held connection was dropped"
        );
        assert_eq!(
            wait_for_stats(&pool, |stats| stats.size == 1 && stats.num_idle == 0).await,
            PoolStats {
                size: 1,
                num_idle: 0,
                is_closed: true,
            }
        );

        drop(held);
        timeout(Duration::from_secs(1), closer)
            .await
            .expect("close timed out")
            .expect("close task panicked");

        assert_eq!(
            pool.stats(),
            PoolStats {
                size: 0,
                num_idle: 0,
                is_closed: true,
            }
        );

        Ok(())
    }
}
