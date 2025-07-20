//! Provides the connection pool for asynchronous connections.
//!
//! Opening a database connection for each and every operation to the database can quickly
//! become expensive. Furthermore, sharing a database connection between threads and functions
//! can be difficult to express in Rust.
//!
//! A connection pool is a standard technique that can manage opening and re-using connections.
//! Normally it also enforces a maximum number of connections as these are an expensive resource
//! on the database server.
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use event_listener::EventListener;
use futures_core::{FusedFuture, future::BoxFuture, stream::BoxStream};
use futures_util::{FutureExt, StreamExt, TryFutureExt, TryStreamExt, future};

use self::inner::PoolInner;
use crate::{
    Either, Error, QueryResult, Result, Row, Statement, executor::Execute, transaction::Transaction,
};

mod connection_like;

mod connection;
mod inner;

pub use self::connection::PoolConnection;
pub use self::connection_like::ConnectionLike;

#[doc(hidden)]
/// An asynchronous pool of database connections.
///
/// Create a pool with [Pool::connect] or [Pool::connect_with] and then call [Pool::acquire] to get a connection from
/// the pool; when the connection is dropped it will return to the pool so it can be reused.
///
/// You can also execute queries directly on `&Pool`; this will automatically checkout a connection
/// for you.
///
/// See [the module documentation](crate::pool) for examples.
///
/// The pool has a maximum connection limit that it will not exceed; if `acquire()` is called when at this limit and all
/// connections are checked out, the task will be made to wait until a connection becomes available.
///
/// You can configure the connection limit, and other parameters, using the
/// [`Musq`](crate::Musq) configuration API.
///
/// Calls to `acquire()` are fair, i.e. fulfilled on a first-come, first-serve basis.
///
/// `Pool` is `Send`, `Sync` and `Clone`. It is intended to be created once at the start of your program and then shared
/// with all tasks throughout the process' lifetime. How best to accomplish this depends on your program architecture.
///
/// Cloning `Pool` is cheap as it is simply a reference-counted handle to the inner pool state. When the last remaining
/// handle to the pool is dropped, the connections owned by the pool are immediately closed (also by dropping).
/// `PoolConnection` returned by [Pool::acquire] and `Transaction` returned by [Pool::begin] both implicitly hold a
/// reference to the pool for their lifetimes.
///
/// We recommend calling [`.close().await`] to gracefully close the pool and its connections when you are done using it.
/// This will also wake any tasks that are waiting on an `.acquire()` call, so for long-lived applications it's a good
/// idea to call `.close()` during shutdown.
///
/// If you're writing tests, consider using `#[test]` which handles the lifetime of the pool for you.
///
/// [`.close().await`]: Pool::close
pub struct Pool(pub(crate) Arc<PoolInner>);

impl Pool {
    pub(crate) async fn new(options: crate::Musq) -> Result<Pool> {
        // Make an initial connection to validate the configuration
        let inner = PoolInner::new_arc(options);
        let conn = inner.acquire().await?;
        inner.release(conn);
        Ok(Pool(inner))
    }
}

/// A future that resolves when the pool is closed.
///
/// See [`Pool::close_event()`] for details.
pub struct CloseEvent {
    listener: Option<EventListener>,
}

impl Pool {
    /// Retrieves a connection from the pool.
    ///
    /// The total time this method is allowed to execute is capped by
    /// [`Musq::acquire_timeout`].
    /// If that timeout elapses, this will return [`Error::PoolClosed`].
    ///
    /// ### Note: Cancellation/Timeout May Drop Connections
    /// If `acquire` is cancelled or times out after it acquires a connection from the idle queue or
    /// opens a new one, it will drop that connection because we don't want to assume it
    /// is safe to return to the pool, and testing it to see if it's safe to release could introduce
    /// subtle bugs if not implemented correctly. To avoid that entirely, we've decided to not
    /// gracefully handle cancellation here.
    ///
    /// However, if your workload is sensitive to dropped connections such as using an in-memory
    /// SQLite database with a pool size of 1, care should be taken to avoid cancelling
    /// `acquire()` calls.
    pub async fn acquire(&self) -> Result<PoolConnection> {
        let shared = self.0.clone();
        shared.acquire().await.map(|conn| conn.reattach())
    }

    /// Attempts to retrieve a connection from the pool if there is one available.
    ///
    /// Returns `None` immediately if there are no idle connections available in the pool
    /// or there are tasks waiting for a connection which have yet to wake.
    pub fn try_acquire(&self) -> Option<PoolConnection> {
        self.0.try_acquire().map(|conn| conn.into_live().reattach())
    }

    /// Retrieves a connection and immediately begins a new transaction.
    pub async fn begin(&self) -> Result<Transaction<PoolConnection>> {
        Transaction::begin(self.acquire().await?).await
    }

    /// Attempts to retrieve a connection and immediately begins a new transaction if successful.
    pub async fn try_begin(&self) -> Result<Option<Transaction<PoolConnection>>> {
        match self.try_acquire() {
            Some(conn) => Transaction::begin(conn).await.map(Some),

            None => Ok(None),
        }
    }

    /// Shut down the connection pool, immediately waking all tasks waiting for a connection.
    ///
    /// Upon calling this method, any currently waiting or subsequent calls to [`Pool::acquire`] and
    /// the like will immediately return [`Error::PoolClosed`] and no new connections will be opened.
    /// Checked-out connections are unaffected, but will be gracefully closed on-drop
    /// rather than being returned to the pool.
    ///
    /// This async method ensures all connections are gracefully closed. It will
    /// first close any idle connections currently waiting in the pool, then wait
    /// for all checked-out connections to be returned or closed.
    ///
    /// Waiting for connections to be gracefully closed is optional, but will allow the database
    /// server to clean up the resources sooner rather than later. This is especially important
    /// for tests that create a new pool every time, otherwise you may see errors about connection
    /// limits being exhausted even when running tests in a single thread.
    ///
    /// If the returned future is not awaited to completion, any remaining
    /// connections will be dropped when the last handle for the given pool
    /// instance is dropped, which could happen in a task spawned by `Pool`
    /// internally and so may be unpredictable otherwise.
    ///
    /// `.close()` may be safely called and `.await`ed on multiple handles concurrently.
    pub async fn close(&self) {
        self.0.close().await
    }

    /// Returns `true` if [`.close()`][Pool::close] has been called on the pool, `false` otherwise.
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// Get a future that resolves when [`Pool::close()`] is called.
    ///
    /// If the pool is already closed, the future resolves immediately.
    ///
    /// This can be used to cancel long-running operations that hold onto a [`PoolConnection`]
    /// so they don't prevent the pool from closing (which would otherwise wait until all
    /// connections are returned).
    pub fn close_event(&self) -> CloseEvent {
        self.0.close_event()
    }

    /// Returns the number of connections currently active. This includes idle connections.
    pub fn size(&self) -> u32 {
        self.0.size()
    }

    /// Returns the number of connections active and idle (not in use).
    pub fn num_idle(&self) -> usize {
        // This previously called [`crossbeam::queue::ArrayQueue::len()`] which waits for the head and tail pointers to
        // be in a consistent state, which may never happen at high levels of churn.
        self.0.num_idle()
    }

    pub fn fetch_many<'c, 'q: 'c, E>(
        &'c self,
        query: E,
    ) -> BoxStream<'c, Result<Either<QueryResult, Row>>>
    where
        E: Execute + 'q,
    {
        let pool = self.clone();

        Box::pin(async_stream::try_stream! {
            let mut conn = pool.acquire().await?;
            let mut s = conn.fetch_many(query);

            while let Some(v) = s.try_next().await? {
                yield v;
            }
        })
    }

    pub fn fetch_optional<'c, 'q: 'c, E>(&'c self, query: E) -> BoxFuture<'c, Result<Option<Row>>>
    where
        E: Execute + 'q,
    {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.fetch_optional(query).await })
    }

    pub fn prepare_with<'c, 'q: 'c>(&'c self, sql: &'q str) -> BoxFuture<'c, Result<Statement>> {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.prepare_with(sql).await })
    }

    pub fn prepare<'c, 'q: 'c>(&'c self, sql: &'q str) -> BoxFuture<'c, Result<Statement>> {
        self.prepare_with(sql)
    }

    pub fn fetch<'c, 'q: 'c, E>(&'c self, query: E) -> BoxStream<'c, Result<Row>>
    where
        E: Execute + 'q,
    {
        self.fetch_many(query)
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(_) => None,
                    Either::Right(row) => Some(row),
                })
            })
            .boxed()
    }

    pub fn execute_many<'c, 'q: 'c, E>(&'c self, query: E) -> BoxStream<'c, Result<QueryResult>>
    where
        E: Execute + 'q,
    {
        self.fetch_many(query)
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(rows) => Some(rows),
                    Either::Right(_) => None,
                })
            })
            .boxed()
    }

    pub fn execute<'c, 'q: 'c, E>(&'c self, query: E) -> BoxFuture<'c, Result<QueryResult>>
    where
        E: Execute + 'q,
    {
        self.execute_many(query).try_collect().boxed()
    }

    pub fn fetch_all<'c, 'q: 'c, E>(&'c self, query: E) -> BoxFuture<'c, Result<Vec<Row>>>
    where
        E: Execute + 'q,
    {
        self.fetch(query).try_collect().boxed()
    }

    pub fn fetch_one<'c, 'q: 'c, E>(&'c self, query: E) -> BoxFuture<'c, Result<Row>>
    where
        E: Execute + 'q,
    {
        self.fetch_optional(query)
            .and_then(|row| match row {
                Some(row) => future::ok(row),
                None => future::err(Error::RowNotFound),
            })
            .boxed()
    }
}

/// Returns a new [Pool] tied to the same shared connection pool.
impl Clone for Pool {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl fmt::Debug for Pool {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Pool")
            .field("size", &self.0.size())
            .field("num_idle", &self.0.num_idle())
            .field("is_closed", &self.0.is_closed())
            .field("options", &self.0.options)
            .finish()
    }
}

impl CloseEvent {
    /// Execute the given future until it returns or the pool is closed.
    ///
    /// Cancels the future and returns `Err(PoolClosed)` if/when the pool is closed.
    /// If the pool was already closed, the future is never run.
    pub async fn do_until<Fut: Future>(&mut self, fut: Fut) -> Result<Fut::Output> {
        // Check that the pool wasn't closed already.
        //
        // We use `poll_immediate()` as it will use the correct waker instead of
        // a no-op one like `.now_or_never()`, but it won't actually suspend execution here.
        futures_util::future::poll_immediate(&mut *self)
            .await
            .map_or(Ok(()), |_| Err(Error::PoolClosed))?;

        futures_util::pin_mut!(fut);

        // I find that this is clearer in intent than `futures_util::future::select()`
        // or `futures_util::select_biased!{}` (which isn't enabled anyway).
        futures_util::future::poll_fn(|cx| {
            // Poll `fut` first as the wakeup event is more likely for it than `self`.
            if let Poll::Ready(ret) = fut.as_mut().poll(cx) {
                return Poll::Ready(Ok(ret));
            }

            // Can't really factor out mapping to `Err(Error::PoolClosed)` though it seems like
            // we should because that results in a different `Ok` type each time.
            //
            // Ideally we'd map to something like `Result<!, Error>` but using `!` as a type
            // is not allowed on stable Rust yet.
            self.poll_unpin(cx).map(|_| Err(Error::PoolClosed))
        })
        .await
    }
}

impl Future for CloseEvent {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(listener) = &mut self.listener {
            futures_core::ready!(listener.poll_unpin(cx));
        }

        // `EventListener` doesn't like being polled after it yields, and even if it did it
        // would probably just wait for the next event, neither of which we want.
        //
        // So this way, once we get our close event, we fuse this future to immediately return.
        self.listener = None;

        Poll::Ready(())
    }
}

impl FusedFuture for CloseEvent {
    fn is_terminated(&self) -> bool {
        self.listener.is_none()
    }
}

#[test]
#[allow(dead_code)]
fn assert_pool_traits() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_clone<T: Clone>() {}

    fn assert_pool() {
        assert_send_sync::<Pool>();
        assert_clone::<Pool>();
    }
}
