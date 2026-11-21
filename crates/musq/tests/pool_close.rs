//! Integration tests for musq.

#[cfg(test)]
mod tests {
    use std::future;

    use musq::{Error, Musq, Pool, PoolStats};
    use tokio::{
        task::yield_now,
        time::{Duration, timeout},
    };

    async fn wait_for_stats(pool: &Pool, predicate: impl Fn(PoolStats) -> bool) -> PoolStats {
        timeout(Duration::from_secs(1), async {
            loop {
                let stats = pool.stats();
                if predicate(stats) {
                    return stats;
                }
                yield_now().await;
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

        let waiter = pool.acquire();
        tokio::pin!(waiter);
        // Poll once so `waiter` registers on the semaphore and close event.
        tokio::select! {
            biased;
            res = &mut waiter => panic!("acquire unexpectedly completed while connection held: {res:?}"),
            _ = future::ready(()) => {}
        }

        let pool_for_close = pool.clone();
        let closer = tokio::spawn(async move {
            let _ = pool_for_close.close().await;
        });

        timeout(Duration::from_secs(1), pool.close_event())
            .await
            .expect("close did not mark pool closed");
        drop(conn); // release the connection so close can finish

        let (close_res, res) = tokio::join!(closer, waiter);
        close_res.expect("close task panicked");
        assert!(matches!(res, Err(Error::PoolClosed)));

        Ok(())
    }

    #[tokio::test]
    async fn close_waits_for_checked_out_connection_after_closing_idle() -> anyhow::Result<()> {
        let pool = Musq::new().max_connections(2).open_in_memory().await?;

        let idle = pool.acquire().await?;
        let held = pool.acquire().await?;
        drop(idle);

        assert_eq!(pool.size(), 2);
        assert_eq!(pool.num_idle(), 1);
        assert!(!pool.is_closed());

        let pool_for_close = pool.clone();
        let closer = tokio::spawn(async move {
            let _ = pool_for_close.close().await;
        });

        assert_eq!(
            wait_for_stats(&pool, |stats| {
                stats.size == 1 && stats.num_idle == 0 && stats.is_closed
            })
            .await,
            PoolStats {
                size: 1,
                num_idle: 0,
                is_closed: true,
            }
        );
        yield_now().await;
        assert!(
            !closer.is_finished(),
            "close returned while a connection was still checked out"
        );
        assert_eq!(pool.size(), 1);
        assert_eq!(pool.num_idle(), 0);
        assert!(pool.is_closed());
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

        assert_eq!(pool.size(), 3);
        assert_eq!(pool.num_idle(), 2);

        let pool_for_close = pool.clone();
        let closer = tokio::spawn(async move {
            let _ = pool_for_close.close().await;
        });

        assert_eq!(
            wait_for_stats(&pool, |stats| {
                stats.size == 1 && stats.num_idle == 0 && stats.is_closed
            })
            .await,
            PoolStats {
                size: 1,
                num_idle: 0,
                is_closed: true,
            }
        );
        yield_now().await;
        assert!(
            !closer.is_finished(),
            "close returned before the held connection was dropped"
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_close_wakes_close_event_without_losing_notification() -> anyhow::Result<()>
    {
        for _ in 0..32 {
            let pool = Musq::new().max_connections(2).open_in_memory().await?;
            let closer_pool = pool.clone();
            let closer = tokio::spawn(async move {
                let _ = closer_pool.close().await;
            });

            timeout(Duration::from_secs(1), pool.close_event())
                .await
                .expect("close_event lost wakeup");
            closer.await.expect("close task panicked");
        }
        Ok(())
    }
}
