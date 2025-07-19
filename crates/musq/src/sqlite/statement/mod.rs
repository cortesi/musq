use std::sync::Arc;

use crate::{Column, IntoArguments, from_row, query, sqlite::Arguments};

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
pub struct Statement {
    pub(crate) sql: String,
    pub columns: Arc<Vec<Column>>,
}

impl Statement {
    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn query(&self) -> query::Query<Arguments> {
        query::query_statement(self)
    }

    pub fn query_with<A>(&self, arguments: A) -> query::Query<A>
    where
        A: IntoArguments,
    {
        query::query_statement_with(self, arguments)
    }

    pub fn query_as<O>(
        &self,
    ) -> query::Map<impl FnMut(crate::Row) -> Result<O, crate::Error> + Send, Arguments>
    where
        O: for<'r> from_row::FromRow<'r> + Send + Unpin,
    {
        query::query_statement_as(self)
    }

    pub fn query_as_with<'s, O, A>(
        &'s self,
        arguments: A,
    ) -> query::Map<impl FnMut(crate::Row) -> Result<O, crate::Error> + Send, A>
    where
        O: for<'r> from_row::FromRow<'r> + Send + Unpin,
        A: IntoArguments,
    {
        query::query_statement_as_with(self, arguments)
    }

    pub fn query_scalar<O>(
        &self,
    ) -> query::Map<impl FnMut(crate::Row) -> Result<O, crate::Error> + Send, Arguments>
    where
        (O,): for<'r> from_row::FromRow<'r>,
        O: Send + Unpin,
    {
        query::query_statement_scalar(self)
    }

    pub fn query_scalar_with<'s, O, A>(
        &'s self,
        arguments: A,
    ) -> query::Map<impl FnMut(crate::Row) -> Result<O, crate::Error> + Send, A>
    where
        (O,): for<'r> from_row::FromRow<'r>,
        O: Send + Unpin,
        A: IntoArguments,
    {
        query::query_statement_scalar_with(self, arguments)
    }
}
