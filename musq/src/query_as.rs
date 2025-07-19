use std::marker::PhantomData;

use crate::{
    Arguments, IntoArguments, QueryResult, Row, Statement,
    encode::Encode,
    error::Error,
    executor::{Execute, Executor},
    from_row::FromRow,
    mapped_query::IntoMapped,
    query::{Map, Query, query, query_statement, query_statement_with, query_with},
};
use either::Either;
use futures_core::stream::BoxStream;

/// Raw SQL query with bind parameters, mapped to a concrete type using [`FromRow`].
/// Returned from [`query_as`][crate::query_as::query_as].
#[must_use = "query must be executed to affect database"]
pub struct QueryAs<O, A> {
    pub(crate) inner: Query<A>,
    pub(crate) output: PhantomData<O>,
}

impl<O: Send, A: Send> Execute for QueryAs<O, A>
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

impl<O> QueryAs<O, Arguments> {
    /// Bind a value for use with this SQL query.
    ///
    /// See [`Query::bind`](Query::bind).
    pub fn bind<'q, T: 'q + Send + Encode>(mut self, value: T) -> Self {
        self.inner = self.inner.bind(value);
        self
    }
}

fn from_row_map<O>(row: Row) -> Result<O, Error>
where
    O: for<'r> FromRow<'r>,
{
    O::from_row("", &row)
}

impl<O, A> IntoMapped<O, A> for QueryAs<O, A>
where
    A: IntoArguments + Send,
    O: Send + Unpin + for<'r> FromRow<'r>,
{
    type Mapper = fn(Row) -> Result<O, Error>;

    fn into_map(self) -> Map<Self::Mapper, A> {
        self.inner.try_map(from_row_map::<O>)
    }
}

impl<O, A> QueryAs<O, A>
where
    A: IntoArguments + Send,
    O: Send + Unpin + for<'r> FromRow<'r>,
{
    /// Execute the query and return the generated results as a stream.
    pub fn fetch<'q, 'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'q + 'e,
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
        O: 'e,
        A: 'q + 'e,
    {
        self.into_map().fetch_many(executor)
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    pub async fn fetch_all<'q, 'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
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

/// Make a SQL query that is mapped to a concrete type
/// using [`FromRow`].
pub fn query_as<'q, O>(sql: &'q str) -> QueryAs<O, Arguments>
where
    O: for<'r> FromRow<'r>,
{
    QueryAs {
        inner: query(sql),
        output: PhantomData,
    }
}

/// Make a SQL query, with the given arguments, that is mapped to a concrete type
/// using [`FromRow`].
pub fn query_as_with<'q, O, A>(sql: &'q str, arguments: A) -> QueryAs<O, A>
where
    A: IntoArguments,
    O: for<'r> FromRow<'r>,
{
    QueryAs {
        inner: query_with(sql, arguments),
        output: PhantomData,
    }
}

// Make a SQL query from a statement, that is mapped to a concrete type.
pub fn query_statement_as<'q, O>(statement: &'q Statement) -> QueryAs<O, Arguments>
where
    O: for<'r> FromRow<'r>,
{
    QueryAs {
        inner: query_statement(statement),
        output: PhantomData,
    }
}

// Make a SQL query from a statement, with the given arguments, that is mapped to a concrete type.
pub fn query_statement_as_with<'q, O, A>(statement: &'q Statement, arguments: A) -> QueryAs<O, A>
where
    A: IntoArguments,
    O: for<'r> FromRow<'r>,
{
    QueryAs {
        inner: query_statement_with(statement, arguments),
        output: PhantomData,
    }
}
