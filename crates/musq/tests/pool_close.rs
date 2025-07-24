use musq::{Error, Musq};
use tokio::time::{Duration, sleep};

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
