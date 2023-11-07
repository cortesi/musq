use std::marker::PhantomData;

use either::Either;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt};

use crate::{
    encode::Encode,
    error::Error,
    executor::{Execute, Executor},
    from_row::FromRow,
    query::{query, query_statement, query_statement_with, query_with, Query},
    types::Type,
    Arguments, IntoArguments, QueryResult, Statement,
};

/// Raw SQL query with bind parameters, mapped to a concrete type using [`FromRow`].
/// Returned from [`query_as`][crate::query_as::query_as].
#[must_use = "query must be executed to affect database"]
pub struct QueryAs<'q, O, A> {
    pub(crate) inner: Query<'q, A>,
    pub(crate) output: PhantomData<O>,
}

impl<'q, O: Send, A: Send> Execute<'q> for QueryAs<'q, O, A>
where
    A: 'q + IntoArguments<'q>,
{
    fn sql(&self) -> &'q str {
        self.inner.sql()
    }

    fn statement(&self) -> Option<&Statement<'q>> {
        self.inner.statement()
    }

    fn take_arguments(&mut self) -> Option<Arguments<'q>> {
        self.inner.take_arguments()
    }

    fn persistent(&self) -> bool {
        self.inner.persistent()
    }
}

impl<'q, O> QueryAs<'q, O, Arguments<'q>> {
    /// Bind a value for use with this SQL query.
    ///
    /// See [`Query::bind`](Query::bind).
    pub fn bind<T: 'q + Send + Encode<'q> + Type>(mut self, value: T) -> Self {
        self.inner = self.inner.bind(value);
        self
    }
}

impl<'q, O, A> QueryAs<'q, O, A> {
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
impl<'q, O, A> QueryAs<'q, O, A>
where
    A: 'q + IntoArguments<'q>,
    O: Send + Unpin + for<'r> FromRow<'r>,
{
    /// Execute the query and return the generated results as a stream.
    pub fn fetch<'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'e,
    {
        self.fetch_many(executor)
            .try_filter_map(|step| async move { Ok(step.right()) })
            .boxed()
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
        O: 'e,
        A: 'e,
    {
        executor
            .fetch_many(self.inner)
            .map(|v| match v {
                Ok(Either::Right(row)) => O::from_row(&row).map(Either::Right),
                Ok(Either::Left(v)) => Ok(Either::Left(v)),
                Err(e) => Err(e),
            })
            .boxed()
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].

    pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'e,
    {
        self.fetch(executor).try_collect().await
    }

    /// Execute the query and returns exactly one row.
    pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<O, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'e,
    {
        self.fetch_optional(executor)
            .await
            .and_then(|row| row.ok_or(Error::RowNotFound))
    }

    /// Execute the query and returns at most one row.
    pub async fn fetch_optional<'e, 'c: 'e, E>(self, executor: E) -> Result<Option<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        O: 'e,
        A: 'e,
    {
        let row = executor.fetch_optional(self.inner).await?;
        if let Some(row) = row {
            O::from_row(&row).map(Some)
        } else {
            Ok(None)
        }
    }
}

/// Make a SQL query that is mapped to a concrete type
/// using [`FromRow`].

pub fn query_as<'q, O>(sql: &'q str) -> QueryAs<'q, O, Arguments<'q>>
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

pub fn query_as_with<'q, O, A>(sql: &'q str, arguments: A) -> QueryAs<'q, O, A>
where
    A: IntoArguments<'q>,
    O: for<'r> FromRow<'r>,
{
    QueryAs {
        inner: query_with(sql, arguments),
        output: PhantomData,
    }
}

// Make a SQL query from a statement, that is mapped to a concrete type.
pub fn query_statement_as<'q, O>(statement: &'q Statement<'q>) -> QueryAs<'q, O, Arguments<'_>>
where
    O: for<'r> FromRow<'r>,
{
    QueryAs {
        inner: query_statement(statement),
        output: PhantomData,
    }
}

// Make a SQL query from a statement, with the given arguments, that is mapped to a concrete type.
pub fn query_statement_as_with<'q, O, A>(
    statement: &'q Statement<'q>,
    arguments: A,
) -> QueryAs<'q, O, A>
where
    A: IntoArguments<'q>,
    O: for<'r> FromRow<'r>,
{
    QueryAs {
        inner: query_statement_with(statement, arguments),
        output: PhantomData,
    }
}
