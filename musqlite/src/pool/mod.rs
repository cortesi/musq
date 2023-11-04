//! Provides the connection pool for asynchronous connections.
//!
//! Opening a database connection for each and every operation to the database can quickly
//! become expensive. Furthermore, sharing a database connection between threads and functions
//! can be difficult to express in Rust.
//!
//! A connection pool is a standard technique that can manage opening and re-using connections.
//! Normally it also enforces a maximum number of connections as these are an expensive resource
//! on the database server.
//!
use self::inner::PoolInner;
use crate::{error::Error, transaction::Transaction, ConnectOptions};

use event_listener::EventListener;
use futures_core::FusedFuture;
use futures_util::FutureExt;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

#[macro_use]
mod executor;

#[macro_use]
pub mod maybe;

mod connection;
mod inner;
mod options;

pub use self::connection::PoolConnection;
pub use self::options::{PoolConnectionMetadata, PoolOptions};

#[doc(hidden)]
pub use self::maybe::MaybePoolConnection;

/// An asynchronous pool of SQLx database connections.
///
/// Create a pool with [Pool::connect] or [Pool::connect_with] and then call [Pool::acquire]
/// to get a connection from the pool; when the connection is dropped it will return to the pool
/// so it can be reused.
///
/// You can also pass `&Pool` directly anywhere an `Executor` is required; this will automatically
/// checkout a connection for you.
///
/// See [the module documentation](crate::pool) for examples.
///
/// The pool has a maximum connection limit that it will not exceed; if `acquire()` is called
/// when at this limit and all connections are checked out, the task will be made to wait until
/// a connection becomes available.
///
/// You can configure the connection limit, and other parameters, using [PoolOptions][crate::pool::PoolOptions].
///
/// Calls to `acquire()` are fair, i.e. fulfilled on a first-come, first-serve basis.
///
/// `Pool` is `Send`, `Sync` and `Clone`. It is intended to be created once at the start of your
/// application/daemon/web server/etc. and then shared with all tasks throughout the process'
/// lifetime. How best to accomplish this depends on your program architecture.
///
/// In Actix-Web, for example, you can share a single pool with all request handlers using [web::Data].
///
/// Cloning `Pool` is cheap as it is simply a reference-counted handle to the inner pool state.
/// When the last remaining handle to the pool is dropped, the connections owned by the pool are
/// immediately closed (also by dropping). `PoolConnection` returned by [Pool::acquire] and
/// `Transaction` returned by [Pool::begin] both implicitly hold a reference to the pool for
/// their lifetimes.
///
/// If you prefer to explicitly shutdown the pool and gracefully close its connections (which
/// depending on the database type, may include sending a message to the database server that the
/// connection is being closed), you can call [Pool::close] which causes all waiting and subsequent
/// calls to [Pool::acquire] to return [Error::PoolClosed], and waits until all connections have
/// been returned to the pool and gracefully closed.
///
/// Type aliases are provided for each database to make it easier to sprinkle `Pool` through
/// your codebase:
///
/// * [SqlitePool][crate::sqlite::SqlitePool] (SQLite)
///
/// [web::Data]: https://docs.rs/actix-web/3/actix_web/web/struct.Data.html
///
/// We recommend calling [`.close().await`] to gracefully close the pool and its connections
/// when you are done using it. This will also wake any tasks that are waiting on an `.acquire()`
/// call, so for long-lived applications it's a good idea to call `.close()` during shutdown.
///
/// If you're writing tests, consider using `#[test]` which handles the lifetime of
/// the pool for you.
///
/// [`.close().await`]: Pool::close
pub struct Pool(pub(crate) Arc<PoolInner>);

/// A future that resolves when the pool is closed.
///
/// See [`Pool::close_event()`] for details.
pub struct CloseEvent {
    listener: Option<EventListener>,
}

impl Pool {
    /// Create a new connection pool with a default pool configuration and
    /// the given connection URL, and immediately establish one connection.
    ///
    /// Refer to the relevant `ConnectOptions` impl for your database for the expected URL format:
    ///
    /// * SQLite: [`SqliteConnectOptions`][crate::sqlite::SqliteConnectOptions]
    ///
    /// The default configuration is mainly suited for testing and light-duty applications.
    /// For production applications, you'll likely want to make at least few tweaks.
    ///
    /// See [`PoolOptions::new()`] for details.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        PoolOptions::new().connect(url).await
    }

    /// Create a new connection pool with a default pool configuration and
    /// the given `ConnectOptions`, and immediately establish one connection.
    ///
    /// The default configuration is mainly suited for testing and light-duty applications.
    /// For production applications, you'll likely want to make at least few tweaks.
    ///
    /// See [`PoolOptions::new()`] for details.
    pub async fn connect_with(options: ConnectOptions) -> Result<Self, Error> {
        PoolOptions::new().connect_with(options).await
    }

    /// Create a new connection pool with a default pool configuration and
    /// the given connection URL.
    ///
    /// The pool will establish connections only as needed.
    ///
    /// Refer to the relevant [`ConnectOptions`] impl for your database for the expected URL format:
    ///
    /// * SQLite: [`SqliteConnectOptions`][crate::sqlite::SqliteConnectOptions]
    ///
    /// The default configuration is mainly suited for testing and light-duty applications.
    /// For production applications, you'll likely want to make at least few tweaks.
    ///
    /// See [`PoolOptions::new()`] for details.
    pub fn connect_lazy(url: &str) -> Result<Self, Error> {
        PoolOptions::new().connect_lazy(url)
    }

    /// Create a new connection pool with a default pool configuration and
    /// the given `ConnectOptions`.
    ///
    /// The pool will establish connections only as needed.
    ///
    /// The default configuration is mainly suited for testing and light-duty applications.
    /// For production applications, you'll likely want to make at least few tweaks.
    ///
    /// See [`PoolOptions::new()`] for details.
    pub fn connect_lazy_with(options: ConnectOptions) -> Self {
        PoolOptions::new().connect_lazy_with(options)
    }

    /// Retrieves a connection from the pool.
    ///
    /// The total time this method is allowed to execute is capped by
    /// [`PoolOptions::acquire_timeout`].
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
    /// SQLite database with a pool size of 1, you can pretty easily ensure that a cancelled
    /// `acquire()` call will never drop connections by tweaking your [`PoolOptions`]:
    ///
    /// * Set [`test_before_acquire(false)`][PoolOptions::test_before_acquire]
    /// * Never set [`before_acquire`][PoolOptions::before_acquire] or
    ///   [`after_connect`][PoolOptions::after_connect].
    ///
    /// This should eliminate any potential `.await` points between acquiring a connection and
    /// returning it.
    pub fn acquire(&self) -> impl Future<Output = Result<PoolConnection, Error>> + 'static {
        let shared = self.0.clone();
        async move { shared.acquire().await.map(|conn| conn.reattach()) }
    }

    /// Attempts to retrieve a connection from the pool if there is one available.
    ///
    /// Returns `None` immediately if there are no idle connections available in the pool
    /// or there are tasks waiting for a connection which have yet to wake.
    pub fn try_acquire(&self) -> Option<PoolConnection> {
        self.0.try_acquire().map(|conn| conn.into_live().reattach())
    }

    /// Retrieves a connection and immediately begins a new transaction.
    pub async fn begin(&self) -> Result<Transaction<'static>, Error> {
        Ok(Transaction::begin(MaybePoolConnection::PoolConnection(self.acquire().await?)).await?)
    }

    /// Attempts to retrieve a connection and immediately begins a new transaction if successful.
    pub async fn try_begin(&self) -> Result<Option<Transaction<'static>>, Error> {
        match self.try_acquire() {
            Some(conn) => Transaction::begin(MaybePoolConnection::PoolConnection(conn))
                .await
                .map(Some),

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
    /// Returns a `Future` which can be `.await`ed to ensure all connections are
    /// gracefully closed. It will first close any idle connections currently waiting in the pool,
    /// then wait for all checked-out connections to be returned or closed.
    ///
    /// Waiting for connections to be gracefully closed is optional, but will allow the database
    /// server to clean up the resources sooner rather than later. This is especially important
    /// for tests that create a new pool every time, otherwise you may see errors about connection
    /// limits being exhausted even when running tests in a single thread.
    ///
    /// If the returned `Future` is not run to completion, any remaining connections will be dropped
    /// when the last handle for the given pool instance is dropped, which could happen in a task
    /// spawned by `Pool` internally and so may be unpredictable otherwise.
    ///
    /// `.close()` may be safely called and `.await`ed on multiple handles concurrently.
    pub fn close(&self) -> impl Future<Output = ()> + '_ {
        self.0.close()
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
    ///
    /// As of 0.6.0, this has been fixed to use a separate atomic counter and so should be fine to
    /// call even at high load.
    ///
    /// This previously called [`crossbeam::queue::ArrayQueue::len()`] which waits for the head and
    /// tail pointers to be in a consistent state, which may never happen at high levels of churn.
    pub fn num_idle(&self) -> usize {
        self.0.num_idle()
    }

    /// Gets a clone of the connection options for this pool
    pub fn connect_options(&self) -> Arc<ConnectOptions> {
        self.0
            .connect_options
            .read()
            .expect("write-lock holder panicked")
            .clone()
    }

    /// Updates the connection options this pool will use when opening any future connections.  Any
    /// existing open connection in the pool will be left as-is.
    pub fn set_connect_options(&self, connect_options: ConnectOptions) {
        // technically write() could also panic if the current thread already holds the lock,
        // but because this method can't be re-entered by the same thread that shouldn't be a problem
        let mut guard = self
            .0
            .connect_options
            .write()
            .expect("write-lock holder panicked");
        *guard = Arc::new(connect_options);
    }

    /// Get the options for this pool
    pub fn options(&self) -> &PoolOptions {
        &self.0.options
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
    pub async fn do_until<Fut: Future>(&mut self, fut: Fut) -> Result<Fut::Output, Error> {
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

/// get the time between the deadline and now and use that as our timeout
///
/// returns `Error::PoolTimedOut` if the deadline is in the past
fn deadline_as_timeout(deadline: Instant) -> Result<Duration, Error> {
    deadline
        .checked_duration_since(Instant::now())
        .ok_or(Error::PoolTimedOut)
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
