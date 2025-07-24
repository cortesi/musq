use async_trait::async_trait;
use either::Either;
use futures_core::stream::BoxStream;
use std::ops::Deref;

use crate::{
    Arguments, QueryResult, Result, Row, encode::Encode, executor::Execute,
    sqlite::statement::Statement,
};

/// Raw SQL query with bind parameters. Returned by [`query`][crate::query::query].
#[must_use = "query must be executed to affect database"]
#[derive(Clone)]
pub struct Query {
    pub(crate) statement: Either<String, Statement>,
    pub(crate) arguments: Option<Arguments>,
    pub(crate) tainted: bool,
}

/// SQL query that will map its results to owned Rust types.
///
/// Returned by [`Query::try_map`], `query!()`, etc. Has most of the same methods as [`Query`] but
/// the return types are changed to reflect the mapping. However, there is no equivalent of
/// [`Query::execute`] as it doesn't make sense to map the result type and then ignore it.
///
/// [`Map::bind`] and [`Map::bind_named`] may be used to add parameters after
/// [`try_map()`]. Stylistically we still recommend placing your `.bind()` calls
/// before `.try_map()` to avoid adding superfluous binds when using
/// `query!()` et al.
#[must_use = "query must be executed to affect database"]
pub struct Map<F> {
    inner: Query,
    mapper: F,
}

// Trait to handle query execution without exposing the old Executor trait
#[async_trait]
pub trait QueryExecutor {
    async fn execute_query(self, query: Query) -> Result<QueryResult>;
    fn fetch_query<'c>(self, query: Query) -> BoxStream<'c, Result<Row>>
    where
        Self: 'c;
    async fn fetch_all_query(self, query: Query) -> Result<Vec<Row>>;
    async fn fetch_one_query(self, query: Query) -> Result<Row>;
    async fn fetch_optional_query(self, query: Query) -> Result<Option<Row>>;
}

// Implement QueryExecutor for &Pool
#[async_trait]
impl QueryExecutor for &crate::Pool {
    async fn execute_query(self, query: Query) -> Result<QueryResult> {
        let conn = self.acquire().await?;
        conn.execute(query).await
    }

    fn fetch_query<'c>(self, query: Query) -> BoxStream<'c, Result<Row>>
    where
        Self: 'c,
    {
        use futures_util::TryStreamExt;
        Box::pin(async_stream::try_stream! {
            let conn = self.acquire().await?;
            let mut stream = conn.fetch(query);
            while let Some(row) = stream.try_next().await? {
                yield row;
            }
        })
    }

    async fn fetch_all_query(self, query: Query) -> Result<Vec<Row>> {
        let conn = self.acquire().await?;
        conn.fetch_all(query).await
    }

    async fn fetch_one_query(self, query: Query) -> Result<Row> {
        let conn = self.acquire().await?;
        conn.fetch_one(query).await
    }

    async fn fetch_optional_query(self, query: Query) -> Result<Option<Row>> {
        let conn = self.acquire().await?;
        conn.fetch_optional(query).await
    }
}

// Implement QueryExecutor for &Connection
#[async_trait]
impl QueryExecutor for &crate::Connection {
    async fn execute_query(self, query: Query) -> Result<QueryResult> {
        self.execute(query).await
    }

    fn fetch_query<'c>(self, query: Query) -> BoxStream<'c, Result<Row>>
    where
        Self: 'c,
    {
        self.fetch(query)
    }

    async fn fetch_all_query(self, query: Query) -> Result<Vec<Row>> {
        self.fetch_all(query).await
    }

    async fn fetch_one_query(self, query: Query) -> Result<Row> {
        self.fetch_one(query).await
    }

    async fn fetch_optional_query(self, query: Query) -> Result<Option<Row>> {
        self.fetch_optional(query).await
    }
}

// Implement QueryExecutor for &PoolConnection
#[async_trait]
impl QueryExecutor for &crate::pool::PoolConnection {
    async fn execute_query(self, query: Query) -> Result<QueryResult> {
        self.execute(query).await
    }

    fn fetch_query<'c>(self, query: Query) -> BoxStream<'c, Result<Row>>
    where
        Self: 'c,
    {
        self.fetch(query)
    }

    async fn fetch_all_query(self, query: Query) -> Result<Vec<Row>> {
        self.fetch_all(query).await
    }

    async fn fetch_one_query(self, query: Query) -> Result<Row> {
        self.fetch_one(query).await
    }

    async fn fetch_optional_query(self, query: Query) -> Result<Option<Row>> {
        self.fetch_optional(query).await
    }
}

// Implement QueryExecutor for &Transaction<C>
#[async_trait]
impl<C> QueryExecutor for &crate::Transaction<C>
where
    C: std::ops::DerefMut<Target = crate::Connection> + Send + Sync,
{
    async fn execute_query(self, query: Query) -> Result<QueryResult> {
        let conn: &crate::Connection = self.deref();
        conn.execute(query).await
    }

    fn fetch_query<'c>(self, query: Query) -> BoxStream<'c, Result<Row>>
    where
        Self: 'c,
    {
        use futures_util::TryStreamExt;
        Box::pin(async_stream::try_stream! {
            let conn: &crate::Connection = self.deref();
            let mut stream = conn.fetch(query);
            while let Some(row) = stream.try_next().await? {
                yield row;
            }
        })
    }

    async fn fetch_all_query(self, query: Query) -> Result<Vec<Row>> {
        let conn: &crate::Connection = self.deref();
        conn.fetch_all(query).await
    }

    async fn fetch_one_query(self, query: Query) -> Result<Row> {
        let conn: &crate::Connection = self.deref();
        conn.fetch_one(query).await
    }

    async fn fetch_optional_query(self, query: Query) -> Result<Option<Row>> {
        let conn: &crate::Connection = self.deref();
        conn.fetch_optional(query).await
    }
}

// Implement QueryExecutor for &mut Transaction<C>
#[async_trait]
impl<C> QueryExecutor for &mut crate::Transaction<C>
where
    C: std::ops::DerefMut<Target = crate::Connection> + Send + Sync,
{
    async fn execute_query(self, query: Query) -> Result<QueryResult> {
        let conn: &crate::Connection = self.deref();
        conn.execute(query).await
    }

    fn fetch_query<'c>(self, query: Query) -> BoxStream<'c, Result<Row>>
    where
        Self: 'c,
    {
        use futures_util::TryStreamExt;
        Box::pin(async_stream::try_stream! {
            let conn: &crate::Connection = self.deref();
            let mut stream = conn.fetch(query);
            while let Some(row) = stream.try_next().await? {
                yield row;
            }
        })
    }

    async fn fetch_all_query(self, query: Query) -> Result<Vec<Row>> {
        let conn: &crate::Connection = self.deref();
        conn.fetch_all(query).await
    }

    async fn fetch_one_query(self, query: Query) -> Result<Row> {
        let conn: &crate::Connection = self.deref();
        conn.fetch_one(query).await
    }

    async fn fetch_optional_query(self, query: Query) -> Result<Option<Row>> {
        let conn: &crate::Connection = self.deref();
        conn.fetch_optional(query).await
    }
}

impl Execute for Query {
    fn sql(&self) -> &str {
        match &self.statement {
            Either::Right(statement) => statement.sql(),
            Either::Left(sql) => sql,
        }
    }

    fn arguments(&self) -> Option<Arguments> {
        self.arguments.clone()
    }
}

impl Query {
    pub fn is_tainted(&self) -> bool {
        self.tainted
    }

    pub fn into_builder(self) -> crate::QueryBuilder {
        crate::QueryBuilder::from_parts(
            self.sql().to_string(),
            self.arguments.unwrap_or_default(),
            self.tainted,
        )
    }

    /// Attempt to bind a value for use with this SQL query.
    ///
    /// If the number of times this is called does not match the number of bind parameters that
    /// appear in the query then an error will be returned when this query is executed.
    pub fn try_bind<'q, T: 'q + Send + Encode>(mut self, value: T) -> Result<Self> {
        if let Some(arguments) = &mut self.arguments {
            arguments.add(value)?;
        }
        Ok(self)
    }

    /// Bind a value for use with this SQL query.
    ///
    /// This will panic if [`try_bind`](Self::try_bind) returns an error.
    pub fn bind<'q, T: 'q + Send + Encode>(self, value: T) -> Self {
        self.try_bind(value)
            .expect("failed to bind query parameter")
    }

    /// Attempt to bind a value to a named parameter.
    pub fn try_bind_named<'q, T: 'q + Send + Encode>(
        mut self,
        name: &str,
        value: T,
    ) -> Result<Self> {
        if let Some(arguments) = &mut self.arguments {
            arguments.add_named(name, value)?;
        }
        Ok(self)
    }

    /// Bind a value to a named parameter.
    ///
    /// This will panic if [`try_bind_named`](Self::try_bind_named) returns an error.
    pub fn bind_named<'q, T: 'q + Send + Encode>(self, name: &str, value: T) -> Self {
        self.try_bind_named(name, value)
            .expect("failed to bind named query parameter")
    }
}

impl<F> Map<F> {
    pub fn try_bind<'q, T: 'q + Send + Encode>(mut self, value: T) -> Result<Self> {
        self.inner = self.inner.try_bind(value)?;
        Ok(self)
    }

    pub fn bind<'q, T: 'q + Send + Encode>(self, value: T) -> Self {
        self.try_bind(value)
            .expect("failed to bind query parameter")
    }

    pub fn try_bind_named<'q, T: 'q + Send + Encode>(
        mut self,
        name: &str,
        value: T,
    ) -> Result<Self> {
        self.inner = self.inner.try_bind_named(name, value)?;
        Ok(self)
    }

    pub fn bind_named<'q, T: 'q + Send + Encode>(self, name: &str, value: T) -> Self {
        self.try_bind_named(name, value)
            .expect("failed to bind named query parameter")
    }
}

impl Query {
    /// Map each row in the result to another type.
    ///
    /// See [`try_map`](Query::try_map) for a fallible version of this method.
    ///
    /// The [`query_as`](crate::query_as) function will construct a mapped query using
    /// a [`FromRow`](crate::FromRow) implementation.
    pub fn map<F, O>(self, mut f: F) -> Map<impl FnMut(Row) -> Result<O> + Send>
    where
        F: FnMut(Row) -> O + Send,
        O: Unpin,
    {
        self.try_map(move |row| Ok(f(row)))
    }

    /// Map each row in the result to another type.
    ///
    /// The [`query_as`](crate::query_as) function will construct a mapped query using
    /// a [`FromRow`](crate::FromRow) implementation.
    pub fn try_map<F, O>(self, f: F) -> Map<F>
    where
        F: FnMut(Row) -> Result<O> + Send,
        O: Unpin,
    {
        Map {
            inner: self,
            mapper: f,
        }
    }

    /// Execute the query and return the total number of rows affected.
    pub async fn execute<E>(self, executor: E) -> Result<QueryResult>
    where
        E: QueryExecutor,
    {
        executor.execute_query(self).await
    }

    /// Execute the query and return the generated results as a stream.
    pub fn fetch<'c, E>(self, executor: E) -> BoxStream<'c, Result<Row>>
    where
        E: QueryExecutor + 'c,
    {
        executor.fetch_query(self)
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    pub async fn fetch_all<E>(self, executor: E) -> Result<Vec<Row>>
    where
        E: QueryExecutor,
    {
        executor.fetch_all_query(self).await
    }

    /// Execute the query and returns exactly one row.
    pub async fn fetch_one<E>(self, executor: E) -> Result<Row>
    where
        E: QueryExecutor,
    {
        executor.fetch_one_query(self).await
    }

    /// Execute the query and returns at most one row.
    pub async fn fetch_optional<E>(self, executor: E) -> Result<Option<Row>>
    where
        E: QueryExecutor,
    {
        executor.fetch_optional_query(self).await
    }
}

impl<F: Send> Execute for Map<F> {
    fn sql(&self) -> &str {
        self.inner.sql()
    }

    fn arguments(&self) -> Option<Arguments> {
        self.inner.arguments()
    }
}

impl<F, O> Map<F>
where
    F: FnMut(Row) -> Result<O> + Send,
    O: Send + Unpin,
{
    /// Map each row in the result to another type.
    ///
    /// See [`try_map`](Map::try_map) for a fallible version of this method.
    ///
    /// The [`query_as`](crate::query_as) function will construct a mapped query using
    /// a [`FromRow`](crate::FromRow) implementation.
    pub fn map<G, P>(self, mut g: G) -> Map<impl FnMut(Row) -> Result<P> + Send>
    where
        G: FnMut(O) -> P + Send,
        P: Unpin,
    {
        self.try_map(move |data| Ok(g(data)))
    }

    /// Map each row in the result to another type.
    ///
    /// The [`query_as`](crate::query_as) function will construct a mapped query using
    /// a [`FromRow`](crate::FromRow) implementation.
    pub fn try_map<G, P>(self, mut g: G) -> Map<impl FnMut(Row) -> Result<P> + Send>
    where
        G: FnMut(O) -> Result<P> + Send,
        P: Unpin,
    {
        let mut f = self.mapper;
        Map {
            inner: self.inner,
            mapper: move |row| f(row).and_then(&mut g),
        }
    }

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    pub async fn fetch_all<E>(mut self, executor: E) -> Result<Vec<O>>
    where
        E: QueryExecutor,
    {
        let rows = self.inner.fetch_all(executor).await?;
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push((self.mapper)(row)?);
        }
        Ok(results)
    }

    /// Execute the query and returns exactly one row.
    pub async fn fetch_one<E>(self, executor: E) -> Result<O>
    where
        E: QueryExecutor,
    {
        match self.fetch_optional(executor).await? {
            Some(row) => Ok(row),
            None => Err(crate::Error::RowNotFound),
        }
    }

    /// Execute the query and returns at most one row.
    pub async fn fetch_optional<E>(mut self, executor: E) -> Result<Option<O>>
    where
        E: QueryExecutor,
    {
        let row = self.inner.fetch_optional(executor).await?;

        if let Some(row) = row {
            (self.mapper)(row).map(Some)
        } else {
            Ok(None)
        }
    }
}

// Make a SQL query from a statement.
pub(crate) fn query_statement(statement: &Statement) -> Query {
    Query {
        arguments: Some(Default::default()),
        statement: Either::Right(statement.clone()),
        tainted: false,
    }
}

// Make a SQL query from a statement, with the given arguments.
pub(crate) fn query_statement_with(statement: &Statement, arguments: Arguments) -> Query {
    Query {
        arguments: Some(arguments),
        statement: Either::Right(statement.clone()),
        tainted: false,
    }
}

/// Make a SQL query.
pub fn query(sql: &str) -> Query {
    Query {
        arguments: Some(Default::default()),
        statement: Either::Left(sql.to_string()),
        tainted: false,
    }
}

/// Make a SQL query, with the given arguments.
pub fn query_with(sql: &str, arguments: Arguments) -> Query {
    Query {
        arguments: Some(arguments),
        statement: Either::Left(sql.to_string()),
        tainted: false,
    }
}

use crate::from_row::FromRow;

pub fn query_as<'q, O>(sql: &'q str) -> Map<impl FnMut(Row) -> Result<O> + Send>
where
    O: Send + Unpin + for<'r> FromRow<'r>,
{
    query(sql).try_map(|row| O::from_row("", &row))
}

pub fn query_as_with<'q, O>(
    sql: &'q str,
    arguments: Arguments,
) -> Map<impl FnMut(Row) -> Result<O> + Send>
where
    O: Send + Unpin + for<'r> FromRow<'r>,
{
    query_with(sql, arguments).try_map(|row| O::from_row("", &row))
}

pub(crate) fn query_statement_as<'q, O>(
    statement: &'q Statement,
) -> Map<impl FnMut(Row) -> Result<O> + Send>
where
    O: Send + Unpin + for<'r> FromRow<'r>,
{
    query_statement(statement).try_map(|row| O::from_row("", &row))
}

pub(crate) fn query_statement_as_with<'q, O>(
    statement: &'q Statement,
    arguments: Arguments,
) -> Map<impl FnMut(Row) -> Result<O> + Send>
where
    O: Send + Unpin + for<'r> FromRow<'r>,
{
    query_statement_with(statement, arguments).try_map(|row| O::from_row("", &row))
}

pub fn query_scalar<'q, O>(sql: &'q str) -> Map<impl FnMut(Row) -> Result<O> + Send>
where
    (O,): for<'r> FromRow<'r>,
    O: Send + Unpin,
{
    query_as(sql).map(|(o,)| o)
}

pub fn query_scalar_with<'q, O>(
    sql: &'q str,
    arguments: Arguments,
) -> Map<impl FnMut(Row) -> Result<O> + Send>
where
    (O,): for<'r> FromRow<'r>,
    O: Send + Unpin,
{
    query_as_with(sql, arguments).map(|(o,)| o)
}

pub(crate) fn query_statement_scalar<'q, O>(
    statement: &'q Statement,
) -> Map<impl FnMut(Row) -> Result<O> + Send>
where
    (O,): for<'r> FromRow<'r>,
    O: Send + Unpin,
{
    query_statement_as(statement).map(|(o,)| o)
}

pub(crate) fn query_statement_scalar_with<'q, O>(
    statement: &'q Statement,
    arguments: Arguments,
) -> Map<impl FnMut(Row) -> Result<O> + Send>
where
    (O,): for<'r> FromRow<'r>,
    O: Send + Unpin,
{
    query_statement_as_with(statement, arguments).map(|(o,)| o)
}

/// Quote an identifier for use in a SQL statement.
pub fn quote_identifier(ident: &str) -> String {
    let escaped = ident.replace('"', "\"\"");
    format!("\"{escaped}\"")
}
