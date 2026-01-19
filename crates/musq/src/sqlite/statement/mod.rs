use crate::{Result, Row, from_row, query, sqlite::Arguments};

/// Compound statement handling.
mod compound;
/// Low-level statement handle wrapper.
mod handle;
/// Unlock notify helpers.
pub(super) mod unlock_notify;

pub use compound::CompoundStatement;
pub use handle::StatementHandle;

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
pub struct Statement {
    /// SQL string for the prepared statement.
    pub(crate) sql: String,
}

impl Statement {
    /// Return the SQL string for this statement.
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

/// A prepared statement without exposed metadata.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// Prepared statement with metadata.
    pub(crate) statement: Statement,
}

impl Prepared {
    /// Return the SQL string for this statement.
    pub fn sql(&self) -> &str {
        self.statement.sql()
    }

    /// Create a query from this prepared statement.
    pub fn query(&self) -> query::Query {
        query::query_statement(&self.statement)
    }

    /// Create a query from this prepared statement with arguments.
    pub fn query_with(&self, arguments: Arguments) -> query::Query {
        query::query_statement_with(&self.statement, arguments)
    }

    /// Create a typed query from this prepared statement.
    pub fn query_as<O>(&self) -> query::Map<impl FnMut(Row) -> Result<O> + Send>
    where
        O: for<'r> from_row::FromRow<'r> + Send + Unpin,
    {
        query::query_statement_as(&self.statement)
    }

    /// Create a typed query from this prepared statement with arguments.
    pub fn query_as_with<'s, O>(
        &'s self,
        arguments: Arguments,
    ) -> query::Map<impl FnMut(Row) -> Result<O> + Send>
    where
        O: for<'r> from_row::FromRow<'r> + Send + Unpin,
    {
        query::query_statement_as_with(&self.statement, arguments)
    }

    /// Create a scalar query from this prepared statement.
    pub fn query_scalar<O>(&self) -> query::Map<impl FnMut(Row) -> Result<O> + Send>
    where
        (O,): for<'r> from_row::FromRow<'r>,
        O: Send + Unpin,
    {
        query::query_statement_scalar(&self.statement)
    }

    /// Create a scalar query from this prepared statement with arguments.
    pub fn query_scalar_with<'s, O>(
        &'s self,
        arguments: Arguments,
    ) -> query::Map<impl FnMut(Row) -> Result<O> + Send>
    where
        (O,): for<'r> from_row::FromRow<'r>,
        O: Send + Unpin,
    {
        query::query_statement_scalar_with(&self.statement, arguments)
    }
}
