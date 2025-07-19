use crate::{
    Arguments, IntoArguments, QueryResult, Row, Statement,
    encode::Encode,
    error::Error,
    executor::{Execute, Executor},
    from_row::FromRow,
    mapped_query::IntoMapped,
    query::Map,
    query_as::{QueryAs, query_as, query_as_with, query_statement_as, query_statement_as_with},
};
use either::Either;
use futures_core::stream::BoxStream;

/// Raw SQL query with bind parameters, mapped to a concrete type using [`FromRow`] on `(O,)`.
/// Returned from [`query_scalar`].
#[must_use = "query must be executed to affect database"]
pub struct QueryScalar<O, A> {
    pub(crate) inner: QueryAs<(O,), A>,
}

impl<O: Send, A: Send> Execute for QueryScalar<O, A>
where
    A: IntoArguments,
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

impl<O> QueryScalar<O, Arguments> {
    /// Bind a value for use with this SQL query.
    ///
    /// See [`Query::bind`](crate::query::Query::bind).
    pub fn bind<'q, T: 'q + Send + Encode>(mut self, value: T) -> Self {
        self.inner = self.inner.bind(value);
        self
    }
}

impl<O, A> QueryScalar<O, A>
where
    O: Send + Unpin,
    A: IntoArguments + Send,
    (O,): Send + Unpin + for<'r> FromRow<'r>,
{
    fn into_map(self) -> Map<impl FnMut(Row) -> Result<O, Error> + Send, A> {
        self.inner.into_map().map(|it| it.0)
    }

    /// Execute the query and return the generated results as a stream.
    pub fn fetch<'q, 'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        A: 'q + 'e,
        O: 'e,
    {
        self.into_map().fetch(executor)
    }

    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    pub fn fetch_many<'q, 'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<Either<QueryResult, O>, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        A: 'q + 'e,
        O: 'e,
    {
        self.into_map().fetch_many(executor)
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    pub async fn fetch_all<'q, 'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        (O,): 'e,
        A: 'q + 'e,
    {
        self.into_map().fetch_all(executor).await
    }

    /// Execute the query and returns exactly one row.
    pub async fn fetch_one<'q, 'e, 'c: 'e, E>(self, executor: E) -> Result<O, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'q + 'e,
    {
        self.into_map().fetch_one(executor).await
    }

    /// Execute the query and returns at most one row.
    pub async fn fetch_optional<'q, 'e, 'c: 'e, E>(self, executor: E) -> Result<Option<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'q + 'e,
    {
        self.into_map().fetch_optional(executor).await
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
