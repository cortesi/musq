use std::marker::PhantomData;

use either::Either;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt};

use crate::{
    arguments::IntoArguments,
    database::{Database, HasStatementCache},
    encode::Encode,
    error::Error,
    executor::{Execute, Executor},
    from_row::FromRow,
    query::{query, query_statement, query_statement_with, query_with, Query},
    sqlite::Sqlite,
    types::Type,
    Arguments, Statement,
};

/// Raw SQL query with bind parameters, mapped to a concrete type using [`FromRow`].
/// Returned from [`query_as`][crate::query_as::query_as].
#[must_use = "query must be executed to affect database"]
pub struct QueryAs<'q, DB: Database, O, A> {
    pub(crate) inner: Query<'q, DB, A>,
    pub(crate) output: PhantomData<O>,
}

impl<'q, DB, O: Send, A: Send> Execute<'q, DB> for QueryAs<'q, DB, O, A>
where
    DB: Database,
    A: 'q + IntoArguments<'q>,
{
    #[inline]
    fn sql(&self) -> &'q str {
        self.inner.sql()
    }

    #[inline]
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

impl<'q, O> QueryAs<'q, Sqlite, O, Arguments<'q>> {
    /// Bind a value for use with this SQL query.
    ///
    /// See [`Query::bind`](Query::bind).
    pub fn bind<T: 'q + Send + Encode<'q, Sqlite> + Type<Sqlite>>(mut self, value: T) -> Self {
        self.inner = self.inner.bind(value);
        self
    }
}

impl<'q, DB, O, A> QueryAs<'q, DB, O, A>
where
    DB: Database + HasStatementCache,
{
    /// If `true`, the statement will get prepared once and cached to the
    /// connection's statement cache.
    ///
    /// If queried once with the flag set to `true`, all subsequent queries
    /// matching the one with the flag will use the cached statement until the
    /// cache is cleared.
    ///
    /// Default: `true`.
    pub fn persistent(mut self, value: bool) -> Self {
        self.inner = self.inner.persistent(value);
        self
    }
}

// FIXME: This is very close, nearly 1:1 with `Map`
// noinspection DuplicatedCode
impl<'q, DB, O, A> QueryAs<'q, DB, O, A>
where
    DB: Database,
    A: 'q + IntoArguments<'q>,
    O: Send + Unpin + for<'r> FromRow<'r, DB::Row>,
{
    /// Execute the query and return the generated results as a stream.
    pub fn fetch<'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
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
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, O>, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
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
    #[inline]
    pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
        O: 'e,
        A: 'e,
    {
        self.fetch(executor).try_collect().await
    }

    /// Execute the query and returns exactly one row.
    pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<O, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
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
        E: 'e + Executor<'c, Database = DB>,
        DB: 'e,
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
#[inline]
pub fn query_as<'q, DB, O>(sql: &'q str) -> QueryAs<'q, DB, O, Arguments<'q>>
where
    DB: Database,
    O: for<'r> FromRow<'r, DB::Row>,
{
    QueryAs {
        inner: query(sql),
        output: PhantomData,
    }
}

/// Make a SQL query, with the given arguments, that is mapped to a concrete type
/// using [`FromRow`].
#[inline]
pub fn query_as_with<'q, DB, O, A>(sql: &'q str, arguments: A) -> QueryAs<'q, DB, O, A>
where
    DB: Database,
    A: IntoArguments<'q>,
    O: for<'r> FromRow<'r, DB::Row>,
{
    QueryAs {
        inner: query_with(sql, arguments),
        output: PhantomData,
    }
}

// Make a SQL query from a statement, that is mapped to a concrete type.
pub fn query_statement_as<'q, DB, O>(
    statement: &'q Statement<'q>,
) -> QueryAs<'q, DB, O, Arguments<'_>>
where
    DB: Database,
    O: for<'r> FromRow<'r, DB::Row>,
{
    QueryAs {
        inner: query_statement(statement),
        output: PhantomData,
    }
}

// Make a SQL query from a statement, with the given arguments, that is mapped to a concrete type.
pub fn query_statement_as_with<'q, DB, O, A>(
    statement: &'q Statement<'q>,
    arguments: A,
) -> QueryAs<'q, DB, O, A>
where
    DB: Database,
    A: IntoArguments<'q>,
    O: for<'r> FromRow<'r, DB::Row>,
{
    QueryAs {
        inner: query_statement_with(statement, arguments),
        output: PhantomData,
    }
}
