use crate::{
    Arguments, error::Result, query_result::QueryResult, row::Row, sqlite::statement::Prepared,
};
use either::Either;
use futures_core::{future::BoxFuture, stream::BoxStream};
use futures_util::{FutureExt, StreamExt, TryFutureExt, TryStreamExt, future};

// Private module that defines the `Sealed` trait used to prevent external
// implementations of [`Execute`].
mod sealed {
    /// Prevent downstream implementations of [`Execute`].
    pub trait Sealed {}

    impl Sealed for &str {}
    impl Sealed for crate::query::Query {}
    impl<F> Sealed for crate::query::Map<F> {}
}

/// A type that may be executed against a database connection.
///
/// This trait is **sealed** and cannot be implemented outside of this crate.
///
/// Implemented for the following:
///
///  * [`&str`](std::str)
///  * [`Query`](super::query::Query)
///  * [`Map<F>`](super::query::Map)
///
pub trait Execute: sealed::Sealed + Send + Sized {
    /// Gets the SQL that will be executed.
    fn sql(&self) -> &str;

    /// Returns the arguments to be bound against the query string.
    ///
    /// Returning `None` for `Arguments` indicates to use a "simple" query protocol and to not
    /// prepare the query. Returning `Some(Default::default())` is an empty arguments object that
    /// will be prepared (and cached) before execution.
    fn arguments(&self) -> Option<Arguments>;

    /// Execute the query and return the total number of rows affected.
    fn execute<'c, E>(self, executor: &'c E) -> BoxFuture<'c, Result<QueryResult>>
    where
        E: Executor<'c>,
        Self: 'c,
    {
        executor.execute(self)
    }

    /// Execute multiple queries and return the rows affected from each query, in a stream.
    fn execute_many<'c, E>(self, executor: &'c E) -> BoxStream<'c, Result<QueryResult>>
    where
        E: Executor<'c>,
        Self: 'c,
    {
        executor.execute_many(self)
    }

    /// Execute the query and return the generated results as a stream.
    fn fetch<'c, E>(self, executor: &'c E) -> BoxStream<'c, Result<Row>>
    where
        E: Executor<'c>,
        Self: 'c,
    {
        executor.fetch(self)
    }

    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    fn fetch_many<'c, E>(
        self,
        executor: &'c E,
    ) -> BoxStream<'c, Result<Either<QueryResult, Row>>>
    where
        E: Executor<'c>,
        Self: 'c,
    {
        executor.fetch_many(self)
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    fn fetch_all<'c, E>(self, executor: &'c E) -> BoxFuture<'c, Result<Vec<Row>>>
    where
        E: Executor<'c>,
        Self: 'c,
    {
        executor.fetch_all(self)
    }

    /// Execute the query and returns exactly one row.
    fn fetch_one<'c, E>(self, executor: &'c E) -> BoxFuture<'c, Result<Row>>
    where
        E: Executor<'c>,
        Self: 'c,
    {
        executor.fetch_one(self)
    }

    /// Execute the query and returns at most one row.
    fn fetch_optional<'c, E>(self, executor: &'c E) -> BoxFuture<'c, Result<Option<Row>>>
    where
        E: Executor<'c>,
        Self: 'c,
    {
        executor.fetch_optional(self)
    }
}

impl Execute for &str {
    fn sql(&self) -> &str {
        self
    }

    fn arguments(&self) -> Option<Arguments> {
        None
    }
}

// Sealed module to prevent external implementations of Executor.
mod sealed_executor {
    pub trait Sealed {}
    impl Sealed for crate::Connection {}
    impl Sealed for crate::Pool {}
    impl Sealed for crate::pool::PoolConnection {}
    impl<C> Sealed for crate::Transaction<C> where
        C: std::ops::DerefMut<Target = crate::Connection> + Send
    {
    }
}

/// A type that can directly execute queries against the database.
///
/// This trait is implemented by [`Connection`], [`Pool`], [`PoolConnection`], and [`Transaction`].
/// It is sealed and cannot be implemented for types outside of `musq`.
pub trait Executor<'c>: sealed_executor::Sealed {
    fn execute<'q, E>(&'c self, query: E) -> BoxFuture<'q, Result<QueryResult>>
    where
        'c: 'q,
        E: Execute + 'q;

    fn fetch_many<'q, E>(&'c self, query: E) -> BoxStream<'q, Result<Either<QueryResult, Row>>>
    where
        'c: 'q,
        E: Execute + 'q;

    fn fetch_optional<'q, E>(&'c self, query: E) -> BoxFuture<'q, Result<Option<Row>>>
    where
        'c: 'q,
        E: Execute + 'q;

    fn prepare_with<'q>(&'c self, sql: &'q str) -> BoxFuture<'q, Result<Prepared>>
    where
        'c: 'q;

    // Default methods
    fn execute_many<'q, E>(&'c self, query: E) -> BoxStream<'q, Result<QueryResult>>
    where
        'c: 'q,
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

    fn fetch<'q, E>(&'c self, query: E) -> BoxStream<'q, Result<Row>>
    where
        'c: 'q,
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

    fn fetch_all<'q, E>(&'c self, query: E) -> BoxFuture<'q, Result<Vec<Row>>>
    where
        'c: 'q,
        E: Execute + 'q,
    {
        self.fetch(query).try_collect().boxed()
    }

    fn fetch_one<'q, E>(&'c self, query: E) -> BoxFuture<'q, Result<Row>>
    where
        'c: 'q,
        E: Execute + 'q,
    {
        self.fetch_optional(query)
            .and_then(|row| match row {
                Some(row) => future::ok(row),
                None => future::err(crate::Error::RowNotFound),
            })
            .boxed()
    }

    fn prepare<'q>(&'c self, sql: &'q str) -> BoxFuture<'q, Result<Prepared>>
    where
        'c: 'q,
    {
        self.prepare_with(sql)
    }
}
