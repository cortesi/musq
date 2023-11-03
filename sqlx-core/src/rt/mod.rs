use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

pub mod rt_tokio;

#[derive(Debug, thiserror::Error)]
#[error("operation timed out")]
pub struct TimeoutError(());

pub enum JoinHandle<T> {
    Tokio(tokio::task::JoinHandle<T>),
    // `PhantomData<T>` requires `T: Unpin`
    _Phantom(PhantomData<fn() -> T>),
}

pub async fn timeout<F: Future>(duration: Duration, f: F) -> Result<F::Output, TimeoutError> {
    return tokio::time::timeout(duration, f)
        .await
        .map_err(|_| TimeoutError(()));
}

pub async fn sleep(duration: Duration) {
    return tokio::time::sleep(duration).await;
}

#[track_caller]
pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let handle = tokio::runtime::Handle::current();
    JoinHandle::Tokio(handle.spawn(fut))
}

#[track_caller]
pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let handle = tokio::runtime::Handle::current();
    JoinHandle::Tokio(handle.spawn_blocking(f))
}

pub async fn yield_now() {
    tokio::task::yield_now().await
}

#[track_caller]
pub fn test_block_on<F: Future>(f: F) -> F::Output {
    return tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to start Tokio runtime")
        .block_on(f);
}

impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = T;

    #[track_caller]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut *self {
            Self::Tokio(handle) => Pin::new(handle)
                .poll(cx)
                .map(|res| res.expect("spawned task panicked")),
            Self::_Phantom(_) => {
                let _ = cx;
                unreachable!("runtime should have been checked on spawn")
            }
        }
    }
}
