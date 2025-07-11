use std::marker::PhantomData;

use either::Either;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt};

use crate::{
    Arguments, IntoArguments, QueryResult, Statement,
    encode::Encode,
    error::Error,
    executor::{Execute, Executor},
    from_row::FromRow,
    query::{Query, query, query_statement, query_statement_with, query_with},
};

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

// FIXME: This is very close, nearly 1:1 with `Map`
// noinspection DuplicatedCode
impl<O, A> QueryAs<O, A>
where
    A: IntoArguments,
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
        self.fetch_many(executor)
            .try_filter_map(|step| async move { Ok(step.right()) })
            .boxed()
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
        executor
            .fetch_many(self.inner)
            .map(|v| match v {
                Ok(Either::Right(row)) => O::from_row("", &row).map(Either::Right),
                Ok(Either::Left(v)) => Ok(Either::Left(v)),
                Err(e) => Err(e),
            })
            .boxed()
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    pub async fn fetch_all<'q, 'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'q + 'e,
    {
        self.fetch(executor).try_collect().await
    }

    /// Execute the query and returns exactly one row.
    pub async fn fetch_one<'q, 'e, 'c: 'e, E>(self, executor: E) -> Result<O, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'q + 'e,
    {
        self.fetch_optional(executor)
            .await
            .and_then(|row| row.ok_or(Error::RowNotFound))
    }

    /// Execute the query and returns at most one row.
    pub async fn fetch_optional<'q, 'e, 'c: 'e, E>(self, executor: E) -> Result<Option<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'q + 'e,
    {
        let row = executor.fetch_optional(self.inner).await?;
        if let Some(row) = row {
            O::from_row("", &row).map(Some)
        } else {
            Ok(None)
        }
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
