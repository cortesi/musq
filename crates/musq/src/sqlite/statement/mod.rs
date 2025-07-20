use crate::{from_row, query, sqlite::Arguments};

mod compound;
mod handle;
pub(super) mod unlock_notify;

pub(crate) use compound::CompoundStatement;
pub(crate) use handle::StatementHandle;

/// An explicitly prepared statement.
///
/// Statements are prepared and cached by default, per connection. This type allows you to
/// look at that cache in-between the statement being prepared and it being executed. This contains
/// the expected columns to be returned and the expected parameter types (if available).
///
/// Statements can be re-used with any connection and on first-use it will be re-prepared and
/// cached within the connection.
#[derive(Debug, Clone)]
#[allow(clippy::rc_buffer)]
pub(crate) struct Statement {
    pub(crate) sql: String,
}

impl Statement {
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

/// A prepared statement without exposed metadata.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub(crate) statement: Statement,
}

impl Prepared {
    pub fn sql(&self) -> &str {
        self.statement.sql()
    }

    pub fn query(&self) -> query::Query {
        query::query_statement(&self.statement)
    }

    pub fn query_with(&self, arguments: Arguments) -> query::Query {
        query::query_statement_with(&self.statement, arguments)
    }

    pub fn query_as<O>(&self) -> query::Map<impl FnMut(crate::Row) -> crate::Result<O> + Send>
    where
        O: for<'r> from_row::FromRow<'r> + Send + Unpin,
    {
        query::query_statement_as(&self.statement)
    }

    pub fn query_as_with<'s, O>(
        &'s self,
        arguments: Arguments,
    ) -> query::Map<impl FnMut(crate::Row) -> crate::Result<O> + Send>
    where
        O: for<'r> from_row::FromRow<'r> + Send + Unpin,
    {
        query::query_statement_as_with(&self.statement, arguments)
    }

    pub fn query_scalar<O>(&self) -> query::Map<impl FnMut(crate::Row) -> crate::Result<O> + Send>
    where
        (O,): for<'r> from_row::FromRow<'r>,
        O: Send + Unpin,
    {
        query::query_statement_scalar(&self.statement)
    }

    pub fn query_scalar_with<'s, O>(
        &'s self,
        arguments: Arguments,
    ) -> query::Map<impl FnMut(crate::Row) -> crate::Result<O> + Send>
    where
        (O,): for<'r> from_row::FromRow<'r>,
        O: Send + Unpin,
    {
        query::query_statement_scalar_with(&self.statement, arguments)
    }
}
