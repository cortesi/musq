

use crate::{
    Arguments, IntoArguments, Statement,
    encode::Encode,
    executor::Execute,
    from_row::FromRow,
    query_common::QueryFetch,
    query_as::{QueryAs, query_as, query_as_with, query_statement_as, query_statement_as_with},
};

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

impl<O, A> QueryFetch for QueryScalar<O, A>
where
    O: Send + Unpin,
    A: IntoArguments,
    (O,): Send + Unpin + for<'r> FromRow<'r>,
{
    type Raw = (O,);
    type Output = O;
    type Args = A;

    fn into_query(self) -> QueryAs<Self::Raw, Self::Args> {
        self.inner
    }

    fn map(raw: Self::Raw) -> Self::Output {
        raw.0
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
