use crate::{error::Error, sqlite, Arguments, QueryResult, Row, Statement};

use either::Either;
use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use futures_util::{future, FutureExt, StreamExt, TryFutureExt, TryStreamExt};
use std::fmt::Debug;

/// A type that contains or can provide a database
/// connection to use for executing queries against the database.
///
/// No guarantees are provided that successive queries run on the same
/// physical database connection.
///
/// A [`Connection`](crate::connection::Connection) is an `Executor` that guarantees that
/// successive queries are ran on the same physical database connection.
///
/// Implemented for the following:
///
///  * [`&Pool`](super::pool::Pool)
///  * [`&mut PoolConnection`](super::pool::PoolConnection)
///  * [`&mut Connection`](super::connection::Connection)
///
pub trait Executor<'c>: Send + Debug + Sized {
    /// Execute the query and return the total number of rows affected.
    fn execute<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<QueryResult, Error>>
    where
        'c: 'e,
        E: Execute<'q> + 'q,
    {
        self.execute_many(query).try_collect().boxed()
    }

    /// Execute multiple queries and return the rows affected from each query, in a stream.
    fn execute_many<'e, 'q: 'e, E>(self, query: E) -> BoxStream<'e, Result<QueryResult, Error>>
    where
        'c: 'e,
        E: Execute<'q> + 'q,
    {
        self.fetch_many(query)
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(rows) => Some(rows),
                    Either::Right(_) => None,
                })
            })
            .boxed()
    }

    /// Execute the query and return the generated results as a stream.
    fn fetch<'e, 'q: 'e, E>(self, query: E) -> BoxStream<'e, Result<Row, Error>>
    where
        'c: 'e,
        E: Execute<'q> + 'q,
    {
        self.fetch_many(query)
            .try_filter_map(|step| async move {
                Ok(match step {
                    Either::Left(_) => None,
                    Either::Right(row) => Some(row),
                })
            })
            .boxed()
    }

    /// Execute multiple queries and return the generated results as a stream
    /// from each query, in a stream.
    fn fetch_many<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> BoxStream<'e, Result<Either<QueryResult, Row>, Error>>
    where
        'c: 'e,
        E: Execute<'q> + 'q;

    /// Execute the query and return all the generated results, collected into a [`Vec`].
    fn fetch_all<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<Vec<Row>, Error>>
    where
        'c: 'e,
        E: Execute<'q> + 'q,
    {
        self.fetch(query).try_collect().boxed()
    }

    /// Execute the query and returns exactly one row.
    fn fetch_one<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<Row, Error>>
    where
        'c: 'e,
        E: Execute<'q> + 'q,
    {
        self.fetch_optional(query)
            .and_then(|row| match row {
                Some(row) => future::ok(row),
                None => future::err(Error::RowNotFound),
            })
            .boxed()
    }

    /// Execute the query and returns at most one row.
    fn fetch_optional<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<Option<Row>, Error>>
    where
        'c: 'e,
        E: Execute<'q> + 'q;

    /// Prepare the SQL query to inspect the type information of its parameters
    /// and results.
    ///
    /// Be advised that when using the `query`, `query_as`, or `query_scalar` functions, the query
    /// is transparently prepared and executed.
    ///
    /// This explicit API is provided to allow access to the statement metadata available after
    /// it prepared but before the first row is returned.

    fn prepare<'e, 'q: 'e>(self, query: &'q str) -> BoxFuture<'e, Result<Statement, Error>>
    where
        'c: 'e,
    {
        self.prepare_with(query, &[])
    }

    /// Prepare the SQL query, with parameter type information, to inspect the
    /// type information about its parameters and results.
    ///
    /// Only some database drivers (PostgreSQL, MSSQL) can take advantage of
    /// this extra information to influence parameter type inference.
    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlite::SqliteDataType],
    ) -> BoxFuture<'e, Result<Statement, Error>>
    where
        'c: 'e;
}

/// A type that may be executed against a database connection.
///
/// Implemented for the following:
///
///  * [`&str`](std::str)
///  * [`Query`](super::query::Query)
///
pub trait Execute<'q>: Send + Sized {
    /// Gets the SQL that will be executed.
    fn sql(&self) -> &'q str;

    /// Gets the previously cached statement, if available.
    fn statement(&self) -> Option<&Statement>;

    /// Returns the arguments to be bound against the query string.
    ///
    /// Returning `None` for `Arguments` indicates to use a "simple" query protocol and to not
    /// prepare the query. Returning `Some(Default::default())` is an empty arguments object that
    /// will be prepared (and cached) before execution.
    fn take_arguments(&mut self) -> Option<Arguments>;
}

impl<'q> Execute<'q> for &'q String {
    fn sql(&self) -> &'q str {
        self
    }

    fn statement(&self) -> Option<&Statement> {
        None
    }

    fn take_arguments(&mut self) -> Option<Arguments> {
        None
    }
}

impl<'q> Execute<'q> for &'q str {
    fn sql(&self) -> &'q str {
        self
    }

    fn statement(&self) -> Option<&Statement> {
        None
    }

    fn take_arguments(&mut self) -> Option<Arguments> {
        None
    }
}

impl<'q> Execute<'q> for (&'q str, Option<Arguments>) {
    fn sql(&self) -> &'q str {
        self.0
    }

    fn statement(&self) -> Option<&Statement> {
        None
    }

    fn take_arguments(&mut self) -> Option<Arguments> {
        self.1.take()
    }
}
