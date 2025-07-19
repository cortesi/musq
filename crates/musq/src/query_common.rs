// Common query functionality shared between QueryAs and QueryScalar

use either::Either;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt, TryFutureExt};

use crate::{
    IntoArguments, QueryResult,
    error::Error,
    executor::Executor,
    query_as::QueryAs,
    from_row::FromRow,
};

/// Trait abstracting the common fetch logic of [`QueryAs`] and [`QueryScalar`].
/// Implementors provide a way to obtain the underlying [`QueryAs`] instance and
/// a mapper from its raw output to the exposed output type.
pub trait QueryFetch: Sized
where
    Self::Raw: Send + Unpin + for<'r> FromRow<'r>,
    Self::Args: IntoArguments,
{
    type Raw;
    type Output;
    type Args;

    /// Consume `self` and return the underlying [`QueryAs`] used for execution.
    fn into_query(self) -> QueryAs<Self::Raw, Self::Args>;

    /// Map the raw output to the exposed output type.
    fn map(raw: Self::Raw) -> Self::Output;

    /// Execute the query and return the generated results as a stream.
    fn fetch<'q, 'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<Self::Output, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        Self::Args: 'q + 'e,
        Self::Output: 'e,
        Self::Raw: 'e,
        Self: 'e,
    {
        self.into_query()
            .fetch(executor)
            .map_ok(Self::map)
            .boxed()
    }

    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    fn fetch_many<'q, 'e, 'c: 'e, E>(self, executor: E)
        -> BoxStream<'e, Result<Either<QueryResult, Self::Output>, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        Self::Args: 'q + 'e,
        Self::Output: 'e,
        Self::Raw: 'e,
        Self: 'e,
    {
        self.into_query()
            .fetch_many(executor)
            .map_ok(|v| v.map_right(Self::map))
            .boxed()
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    async fn fetch_all<'q, 'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<Self::Output>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        Self::Args: 'q + 'e,
        Self::Output: 'e,
        Self::Raw: 'e,
        Self: 'e,
    {
        self.into_query()
            .fetch(executor)
            .map_ok(Self::map)
            .try_collect()
            .await
    }

    /// Execute the query and returns exactly one row.
    async fn fetch_one<'q, 'e, 'c: 'e, E>(self, executor: E) -> Result<Self::Output, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        Self::Args: 'q + 'e,
        Self::Output: 'e,
        Self::Raw: 'e,
        Self: 'e,
    {
        self.into_query()
            .fetch_one(executor)
            .map_ok(Self::map)
            .await
    }

    /// Execute the query and returns at most one row.
    async fn fetch_optional<'q, 'e, 'c: 'e, E>(self, executor: E)
        -> Result<Option<Self::Output>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        Self::Args: 'q + 'e,
        Self::Output: 'e,
        Self::Raw: 'e,
        Self: 'e,
    {
        Ok(
            self
                .into_query()
                .fetch_optional(executor)
                .await?
                .map(Self::map),
        )
    }
}

