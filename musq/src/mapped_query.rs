use crate::{IntoArguments, QueryResult, Row, error::Error, executor::Executor, query::Map};
use either::Either;
use futures_core::{future::BoxFuture, stream::BoxStream};

/// Helper trait for query types that can be represented as [`Map`].
///
/// This provides default implementations of the standard fetch methods by
/// converting `self` into a [`Map`] and delegating the call.
pub trait IntoMapped<O, A>
where
    A: Send + IntoArguments,
    O: Send + Unpin,
{
    /// Mapping function type.
    type Mapper: FnMut(Row) -> Result<O, Error> + Send;

    /// Convert this query wrapper into [`Map`].
    fn into_map(self) -> Map<Self::Mapper, A>;

    /// Execute the query and return the generated results as a stream.
    fn fetch<'q, 'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        A: 'q + 'e,
        Self::Mapper: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        Self: Sized,
    {
        self.into_map().fetch(executor)
    }

    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    fn fetch_many<'q, 'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<Either<QueryResult, O>, Error>>
    where
        'q: 'e,
        A: 'q + 'e,
        Self::Mapper: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        Self: Sized,
    {
        self.into_map().fetch_many(executor)
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    fn fetch_all<'q, 'e, 'c: 'e, E>(self, executor: E) -> BoxFuture<'e, Result<Vec<O>, Error>>
    where
        'q: 'e,
        A: 'q + 'e,
        Self::Mapper: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        Self: Sized,
    {
        Box::pin(self.into_map().fetch_all(executor))
    }

    /// Execute the query and returns exactly one row.
    fn fetch_one<'q, 'e, 'c: 'e, E>(self, executor: E) -> BoxFuture<'e, Result<O, Error>>
    where
        'q: 'e,
        A: 'q + 'e,
        Self::Mapper: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        Self: Sized,
    {
        Box::pin(self.into_map().fetch_one(executor))
    }

    /// Execute the query and returns at most one row.
    fn fetch_optional<'q, 'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> BoxFuture<'e, Result<Option<O>, Error>>
    where
        'q: 'e,
        A: 'q + 'e,
        Self::Mapper: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        Self: Sized,
    {
        Box::pin(self.into_map().fetch_optional(executor))
    }
}
