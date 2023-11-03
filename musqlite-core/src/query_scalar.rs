use either::Either;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryFutureExt, TryStreamExt};

use crate::{
    arguments::IntoArguments,
    encode::Encode,
    error::Error,
    executor::{Execute, Executor},
    from_row::FromRow,
    query_as::{query_as, query_as_with, query_statement_as, query_statement_as_with, QueryAs},
    types::Type,
    Arguments, QueryResult, Statement,
};

/// Raw SQL query with bind parameters, mapped to a concrete type using [`FromRow`] on `(O,)`.
/// Returned from [`query_scalar`].
#[must_use = "query must be executed to affect database"]
pub struct QueryScalar<'q, O, A> {
    pub(crate) inner: QueryAs<'q, (O,), A>,
}

impl<'q, O: Send, A: Send> Execute<'q> for QueryScalar<'q, O, A>
where
    A: 'q + IntoArguments<'q>,
{
    #[inline]
    fn sql(&self) -> &'q str {
        self.inner.sql()
    }

    fn statement(&self) -> Option<&Statement<'q>> {
        self.inner.statement()
    }

    #[inline]
    fn take_arguments(&mut self) -> Option<Arguments<'q>> {
        self.inner.take_arguments()
    }

    #[inline]
    fn persistent(&self) -> bool {
        self.inner.persistent()
    }
}

impl<'q, O> QueryScalar<'q, O, Arguments<'q>> {
    /// Bind a value for use with this SQL query.
    ///
    /// See [`Query::bind`](crate::query::Query::bind).
    pub fn bind<T: 'q + Send + Encode<'q> + Type>(mut self, value: T) -> Self {
        self.inner = self.inner.bind(value);
        self
    }
}

impl<'q, O, A> QueryScalar<'q, O, A> {
    /// If `true`, the statement will get prepared once and cached to the
    /// connection's statement cache.
    ///
    /// If queried once with the flag set to `true`, all subsequent queries
    /// matching the one with the flag will use the cached statement until the
    /// cache is cleared.
    ///
    /// Default: `true`.
    pub fn persist(mut self, value: bool) -> Self {
        self.inner = self.inner.persist(value);
        self
    }
}

// FIXME: This is very close, nearly 1:1 with `Map`
// noinspection DuplicatedCode
impl<'q, O, A> QueryScalar<'q, O, A>
where
    O: Send + Unpin,
    A: 'q + IntoArguments<'q>,
    (O,): Send + Unpin + for<'r> FromRow<'r>,
{
    /// Execute the query and return the generated results as a stream.
    #[inline]
    pub fn fetch<'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        A: 'e,
        O: 'e,
    {
        self.inner.fetch(executor).map_ok(|it| it.0).boxed()
    }

    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    #[inline]
    pub fn fetch_many<'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<Either<QueryResult, O>, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        A: 'e,
        O: 'e,
    {
        self.inner
            .fetch_many(executor)
            .map_ok(|v| v.map_right(|it| it.0))
            .boxed()
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    #[inline]
    pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        (O,): 'e,
        A: 'e,
    {
        self.inner
            .fetch(executor)
            .map_ok(|it| it.0)
            .try_collect()
            .await
    }

    /// Execute the query and returns exactly one row.
    #[inline]
    pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<O, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'e,
    {
        self.inner.fetch_one(executor).map_ok(|it| it.0).await
    }

    /// Execute the query and returns at most one row.
    #[inline]
    pub async fn fetch_optional<'e, 'c: 'e, E>(self, executor: E) -> Result<Option<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'e,
    {
        Ok(self.inner.fetch_optional(executor).await?.map(|it| it.0))
    }
}

/// Make a SQL query that is mapped to a single concrete type
/// using [`FromRow`].
#[inline]
pub fn query_scalar<'q, O>(sql: &'q str) -> QueryScalar<'q, O, Arguments<'q>>
where
    (O,): for<'r> FromRow<'r>,
{
    QueryScalar {
        inner: query_as(sql),
    }
}

/// Make a SQL query, with the given arguments, that is mapped to a single concrete type
/// using [`FromRow`].
#[inline]
pub fn query_scalar_with<'q, O, A>(sql: &'q str, arguments: A) -> QueryScalar<'q, O, A>
where
    A: IntoArguments<'q>,
    (O,): for<'r> FromRow<'r>,
{
    QueryScalar {
        inner: query_as_with(sql, arguments),
    }
}

// Make a SQL query from a statement, that is mapped to a concrete value.
pub fn query_statement_scalar<'q, O>(
    statement: &'q Statement<'q>,
) -> QueryScalar<'q, O, Arguments<'_>>
where
    (O,): for<'r> FromRow<'r>,
{
    QueryScalar {
        inner: query_statement_as(statement),
    }
}

// Make a SQL query from a statement, with the given arguments, that is mapped to a concrete value.
pub fn query_statement_scalar_with<'q, O, A>(
    statement: &'q Statement<'q>,
    arguments: A,
) -> QueryScalar<'q, O, A>
where
    A: IntoArguments<'q>,
    (O,): for<'r> FromRow<'r>,
{
    QueryScalar {
        inner: query_statement_as_with(statement, arguments),
    }
}
