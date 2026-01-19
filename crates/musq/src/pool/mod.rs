//! Provides the connection pool for asynchronous connections.
//!
//! Opening a database connection for each and every operation to the database can quickly
//! become expensive. Furthermore, sharing a database connection between threads and functions
//! can be difficult to express in Rust.
//!
//! A connection pool is a standard technique that can manage opening and re-using connections.
//! Normally it also enforces a maximum number of connections as these are an expensive resource,
//! even when working with SQLite.
use std::{fmt, future::Future, sync::Arc};

use self::inner::PoolInner;
use crate::{Result, transaction::Transaction};

/// Pool connection wrappers and lifecycle helpers.
mod connection;
/// Internal pool state and scheduling.
mod inner;

pub use self::connection::PoolConnection;

/// An asynchronous pool of database connections.
///
/// The `Pool` is the main entry point for interacting with a SQLite database. It manages a set
/// of connections, allowing multiple asynchronous tasks to execute queries concurrently.
///
/// For most use cases, you can execute queries directly on a shared reference to the pool (`&Pool`).
/// This is ideal for stateless, one-off queries, as the pool will handle acquiring and releasing
/// a connection for you.
///
/// The `Pool` is `Send`, `Sync`, and cheap to clone. It should be created once when your
/// application starts and then shared across all tasks.
///
/// ## Transactions
///
/// To run a series of queries within a transaction, call [`pool.begin()`][Pool::begin] to acquire
/// a [`Transaction`] object. All operations on the `Transaction` object are guaranteed to run
/// on the same underlying connection.
///
/// ## Connection-Specific State
///
/// In rare cases, you may need to run a series of non-transactional queries that rely on
/// connection-specific state, such as temporary tables. For these scenarios, you can manually
/// acquire a [`PoolConnection`] from the pool using [`pool.acquire()`][Pool::acquire].
///
/// See [`PoolConnection`] for more details on this advanced use case.
///
pub struct Pool(pub(crate) Arc<PoolInner>);

impl Pool {
    /// Create a new connection pool from the provided options.
    pub(crate) async fn new(options: crate::Musq) -> Result<Self> {
        // Make an initial connection to validate the configuration
        let inner = PoolInner::new_arc(options);
        let conn = inner.acquire().await?;
        inner.release(conn);
        Ok(Self(inner))
    }

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
    /// Waiting for connections to be gracefully closed is optional, but will allow SQLite to
    /// clean up resources sooner rather than later. This is especially important for tests that
    /// create a new pool every time, otherwise you may see errors about connection limits being
    /// exhausted even when running tests in a single thread.
    ///
    /// If the returned future is not awaited to completion, any remaining
    /// connections will be dropped when the last handle for the given pool
    /// instance is dropped, which could happen in a task spawned by `Pool`
    /// internally and so may be unpredictable otherwise.
    ///
    /// `.close()` may be safely called and `.await`ed on multiple handles concurrently.
    ///
    /// The returned future **must** be awaited to ensure the pool is fully
    /// closed.
    #[must_use = "futures returned by `Pool::close` must be awaited"]
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
    pub fn close_event(&self) -> impl Future<Output = ()> + '_ {
        self.0.close_event()
    }

    /// Returns the number of connections currently active. This includes idle connections.
    pub(crate) fn size(&self) -> u32 {
        self.0.size()
    }

    /// Returns the number of connections active and idle (not in use).
    pub(crate) fn num_idle(&self) -> usize {
        // This previously called [`crossbeam::queue::ArrayQueue::len()`] which waits for the head and tail pointers to
        // be in a consistent state, which may never happen at high levels of churn.
        self.0.num_idle()
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
            .field("size", &self.size())
            .field("num_idle", &self.num_idle())
            .field("is_closed", &self.0.is_closed())
            .field("options", &self.0.options)
            .finish()
    }
}
