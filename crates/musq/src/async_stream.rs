use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::future::BoxFuture;
use futures_core::stream::Stream;
use futures_util::{FutureExt, pin_mut};
use tokio::sync::mpsc;

use crate::error::Error;

pub struct TryAsyncStream<'a, T> {
    receiver: mpsc::Receiver<Result<T, Error>>,
    future: BoxFuture<'a, Result<(), Error>>,
}

impl<'a, T> TryAsyncStream<'a, T> {
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: FnOnce(mpsc::Sender<Result<T, Error>>) -> Fut + Send,
        Fut: 'a + Future<Output = Result<(), Error>> + Send,
        T: 'a + Send,
    {
        let (sender, receiver) = mpsc::channel(1);
        let err_sender = sender.clone();

        let future = f(sender);
        let future = async move {
            if let Err(error) = future.await {
                let _ = err_sender.send(Err(error)).await;
            }

            Ok(())
        }
        .fuse()
        .boxed();

        Self { future, receiver }
    }
}

impl<'a, T> Stream for TryAsyncStream<'a, T> {
    type Item = Result<T, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let future = &mut self.future;
        pin_mut!(future);

        // the future is fused so its safe to call forever
        // the future advances our "stream"
        // the future should be polled in tandem with the stream receiver
        let _ = future.poll(cx);

        let mut receiver = Pin::new(&mut self.receiver);

        // then we check to see if we have anything to return
        receiver.poll_recv(cx)
    }
}

#[macro_export]
macro_rules! try_stream {
    ($($block:tt)*) => {
        $crate::async_stream::TryAsyncStream::new(move |sender| async move {
            macro_rules! r#yield {
                ($v:expr) => {{
                    let _ = sender.send(Ok($v)).await;
                }}
            }

            $($block)*
        })
    }
}
