use either::Either;
use futures_core::stream::BoxStream;
use futures_util::{future, StreamExt, TryFutureExt, TryStreamExt};

use crate::{
    encode::Encode,
    error::Error,
    executor::{Execute, Executor},
    Arguments, IntoArguments, QueryResult, Row, Statement,
};

/// Raw SQL query with bind parameters. Returned by [`query`][crate::query::query].
#[must_use = "query must be executed to affect database"]
pub struct Query<'q, A> {
    pub(crate) statement: Either<&'q str, &'q Statement>,
    pub(crate) arguments: Option<A>,
}

/// SQL query that will map its results to owned Rust types.
///
/// Returned by [`Query::try_map`], `query!()`, etc. Has most of the same methods as [`Query`] but
/// the return types are changed to reflect the mapping. However, there is no equivalent of
/// [`Query::execute`] as it doesn't make sense to map the result type and then ignore it.
///
/// [`Query::bind`] is also omitted; stylistically we recommend placing your `.bind()` calls
/// before `.try_map()`. This is also to prevent adding superfluous binds to the result of
/// `query!()` et al.
#[must_use = "query must be executed to affect database"]
pub struct Map<'q, F, A> {
    inner: Query<'q, A>,
    mapper: F,
}

impl<'q, A> Execute<'q> for Query<'q, A>
where
    A: Send + IntoArguments,
{
    fn sql(&self) -> &'q str {
        match self.statement {
            Either::Right(statement) => statement.sql(),
            Either::Left(sql) => sql,
        }
    }

    fn statement(&self) -> Option<&Statement> {
        match self.statement {
            Either::Right(statement) => Some(statement),
            Either::Left(_) => None,
        }
    }

    fn take_arguments(&mut self) -> Option<Arguments> {
        self.arguments.take().map(IntoArguments::into_arguments)
    }
}

impl<'q> Query<'q, Arguments> {
    /// Bind a value for use with this SQL query.
    ///
    /// If the number of times this is called does not match the number of bind parameters that appear in the query then
    /// an error will be returned when this query is executed.
    pub fn bind<T: 'q + Send + Encode>(mut self, value: T) -> Self {
        if let Some(arguments) = &mut self.arguments {
            arguments.add(value);
        }
        self
    }
}

impl<'q, A: Send> Query<'q, A>
where
    A: 'q + IntoArguments,
{
    /// Map each row in the result to another type.
    ///
    /// See [`try_map`](Query::try_map) for a fallible version of this method.
    ///
    /// The [`query_as`](super::query_as::query_as) method will construct a mapped query using
    /// a [`FromRow`](super::from_row::FromRow) implementation.

    pub fn map<F, O>(self, mut f: F) -> Map<'q, impl FnMut(Row) -> Result<O, Error> + Send, A>
    where
        F: FnMut(Row) -> O + Send,
        O: Unpin,
    {
        self.try_map(move |row| Ok(f(row)))
    }

    /// Map each row in the result to another type.
    ///
    /// The [`query_as`](super::query_as::query_as) method will construct a mapped query using
    /// a [`FromRow`](super::from_row::FromRow) implementation.

    pub fn try_map<F, O>(self, f: F) -> Map<'q, F, A>
    where
        F: FnMut(Row) -> Result<O, Error> + Send,
        O: Unpin,
    {
        Map {
            inner: self,
            mapper: f,
        }
    }

    /// Execute the query and return the total number of rows affected.

    pub async fn execute<'e, 'c: 'e, E>(self, executor: E) -> Result<QueryResult, Error>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c>,
    {
        executor.execute(self).await
    }

    /// Execute multiple queries and return the rows affected from each query, in a stream.

    pub async fn execute_many<'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<QueryResult, Error>>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c>,
    {
        executor.execute_many(self)
    }

    /// Execute the query and return the generated results as a stream.

    pub fn fetch<'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<Row, Error>>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c>,
    {
        executor.fetch(self)
    }

    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.

    pub fn fetch_many<'e, 'c: 'e, E>(
        self,
        executor: E,
    ) -> BoxStream<'e, Result<Either<QueryResult, Row>, Error>>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c>,
    {
        executor.fetch_many(self)
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].

    pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<Row>, Error>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c>,
    {
        executor.fetch_all(self).await
    }

    /// Execute the query and returns exactly one row.

    pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<Row, Error>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c>,
    {
        executor.fetch_one(self).await
    }

    /// Execute the query and returns at most one row.

    pub async fn fetch_optional<'e, 'c: 'e, E>(self, executor: E) -> Result<Option<Row>, Error>
    where
        'q: 'e,
        A: 'e,
        E: Executor<'c>,
    {
        executor.fetch_optional(self).await
    }
}

impl<'q, F: Send, A: Send> Execute<'q> for Map<'q, F, A>
where
    A: IntoArguments,
{
    fn sql(&self) -> &'q str {
        self.inner.sql()
    }

    fn statement(&self) -> Option<&Statement> {
        self.inner.statement()
    }

    fn take_arguments(&mut self) -> Option<Arguments> {
        self.inner.take_arguments()
    }
}

impl<'q, F, O, A> Map<'q, F, A>
where
    F: FnMut(Row) -> Result<O, Error> + Send,
    O: Send + Unpin,
    A: 'q + Send + IntoArguments,
{
    /// Map each row in the result to another type.
    ///
    /// See [`try_map`](Map::try_map) for a fallible version of this method.
    ///
    /// The [`query_as`](super::query_as::query_as) method will construct a mapped query using
    /// a [`FromRow`](super::from_row::FromRow) implementation.

    pub fn map<G, P>(self, mut g: G) -> Map<'q, impl FnMut(Row) -> Result<P, Error> + Send, A>
    where
        G: FnMut(O) -> P + Send,
        P: Unpin,
    {
        self.try_map(move |data| Ok(g(data)))
    }

    /// Map each row in the result to another type.
    ///
    /// The [`query_as`](super::query_as::query_as) method will construct a mapped query using
    /// a [`FromRow`](super::from_row::FromRow) implementation.

    pub fn try_map<G, P>(self, mut g: G) -> Map<'q, impl FnMut(Row) -> Result<P, Error> + Send, A>
    where
        G: FnMut(O) -> Result<P, Error> + Send,
        P: Unpin,
    {
        let mut f = self.mapper;
        Map {
            inner: self.inner,
            mapper: move |row| f(row).and_then(&mut g),
        }
    }

    /// Execute the query and return the generated results as a stream.
    pub fn fetch<'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        F: 'e,
        O: 'e,
    {
        self.fetch_many(executor)
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(_) => None,
                    Either::Right(o) => Some(o),
                })
            })
            .boxed()
    }

    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    pub fn fetch_many<'e, 'c: 'e, E>(
        mut self,
        executor: E,
    ) -> BoxStream<'e, Result<Either<QueryResult, O>, Error>>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        F: 'e,
        O: 'e,
    {
        Box::pin(try_stream! {
            let mut s = executor.fetch_many(self.inner);

            while let Some(v) = s.try_next().await? {
                r#yield!(match v {
                    Either::Left(v) => Either::Left(v),
                    Either::Right(row) => {
                        Either::Right((self.mapper)(row)?)
                    }
                });
            }

            Ok(())
        })
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        F: 'e,
        O: 'e,
    {
        self.fetch(executor).try_collect().await
    }

    /// Execute the query and returns exactly one row.
    pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<O, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        F: 'e,
        O: 'e,
    {
        self.fetch_optional(executor)
            .and_then(|row| match row {
                Some(row) => future::ok(row),
                None => future::err(Error::RowNotFound),
            })
            .await
    }

    /// Execute the query and returns at most one row.
    pub async fn fetch_optional<'e, 'c: 'e, E>(mut self, executor: E) -> Result<Option<O>, Error>
    where
        'q: 'e,
        E: 'e + Executor<'c>,
        F: 'e,
        O: 'e,
    {
        let row = executor.fetch_optional(self.inner).await?;

        if let Some(row) = row {
            (self.mapper)(row).map(Some)
        } else {
            Ok(None)
        }
    }
}

// Make a SQL query from a statement.
pub fn query_statement<'q>(statement: &'q Statement) -> Query<'q, Arguments> {
    Query {
        arguments: Some(Default::default()),
        statement: Either::Right(statement),
    }
}

// Make a SQL query from a statement, with the given arguments.
pub fn query_statement_with<'q, A>(statement: &'q Statement, arguments: A) -> Query<'q, A>
where
    A: IntoArguments,
{
    Query {
        arguments: Some(arguments),
        statement: Either::Right(statement),
    }
}

/// Make a SQL query.
pub fn query(sql: &str) -> Query<'_, Arguments> {
    Query {
        arguments: Some(Default::default()),
        statement: Either::Left(sql),
    }
}

/// Make a SQL query, with the given arguments.
pub fn query_with<A>(sql: &str, arguments: A) -> Query<'_, A>
where
    A: IntoArguments,
{
    Query {
        arguments: Some(arguments),
        statement: Either::Left(sql),
    }
}
