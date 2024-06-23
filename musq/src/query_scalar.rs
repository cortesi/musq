use either::Either;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryFutureExt, TryStreamExt};

use crate::{
    encode::Encode,
    error::Error,
    executor::{Execute, Executor},
    from_row::FromRow,
    query_as::{query_as, query_as_with, query_statement_as, query_statement_as_with, QueryAs},
    Arguments, IntoArguments, QueryResult, Statement,
};

/// Raw SQL query with bind parameters, mapped to a concrete type using [`FromRow`] on `(O,)`.
/// Returned from [`query_scalar`].
#[must_use = "query must be executed to affect database"]
pub struct QueryScalar<O, A> {
    pub(crate) inner: QueryAs<(O,), A>,
}

impl<'q, O: Send, A: Send> Execute for QueryScalar<O, A>
where
    A: 'q + IntoArguments,
{
    fn sql(&self) -> &str {
        self.inner.sql()
    }

    fn statement(&self) -> Option<&Statement> {
        self.inner.statement()
    }

    fn take_arguments(&mut self) -> Option<Arguments> {
        self.inner.take_arguments()
    }
}

impl<'q, O> QueryScalar<O, Arguments> {
    /// Bind a value for use with this SQL query.
    ///
    /// See [`Query::bind`](crate::query::Query::bind).
    pub fn bind<T: 'q + Send + Encode>(mut self, value: T) -> Self {
        self.inner = self.inner.bind(value);
        self
    }
}

// FIXME: This is very close, nearly 1:1 with `Map`
// noinspection DuplicatedCode
impl<'q, O, A> QueryScalar<O, A>
where
    O: Send + Unpin,
    A: 'q + IntoArguments,
    (O,): Send + Unpin + for<'r> FromRow<'r>,
{
    /// Execute the query and return the generated results as a stream.

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

pub fn query_scalar<'q, O>(sql: &'q str) -> QueryScalar<O, Arguments>
where
    (O,): for<'r> FromRow<'r>,
{
    QueryScalar {
        inner: query_as(sql),
    }
}

/// Make a SQL query, with the given arguments, that is mapped to a single concrete type
/// using [`FromRow`].

pub fn query_scalar_with<'q, O, A>(sql: &'q str, arguments: A) -> QueryScalar<O, A>
where
    A: IntoArguments,
    (O,): for<'r> FromRow<'r>,
{
    QueryScalar {
        inner: query_as_with(sql, arguments),
    }
}

// Make a SQL query from a statement, that is mapped to a concrete value.
pub fn query_statement_scalar<'q, O>(statement: &'q Statement) -> QueryScalar<O, Arguments>
where
    (O,): for<'r> FromRow<'r>,
{
    QueryScalar {
        inner: query_statement_as(statement),
    }
}

// Make a SQL query from a statement, with the given arguments, that is mapped to a concrete value.
pub fn query_statement_scalar_with<'q, O, A>(
    statement: &'q Statement,
    arguments: A,
) -> QueryScalar<O, A>
where
    A: IntoArguments,
    (O,): for<'r> FromRow<'r>,
{
    QueryScalar {
        inner: query_statement_as_with(statement, arguments),
    }
}
