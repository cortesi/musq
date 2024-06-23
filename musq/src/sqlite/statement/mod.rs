use std::sync::Arc;

use crate::{from_row, query, query_as, query_scalar, sqlite::Arguments, Column, IntoArguments};

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

    pub fn query_as<O>(&self) -> query_as::QueryAs<O, Arguments>
    where
        O: for<'r> from_row::FromRow<'r>,
    {
        query_as::query_statement_as(self)
    }

    pub fn query_as_with<'s, O, A>(&'s self, arguments: A) -> query_as::QueryAs<O, A>
    where
        O: for<'r> from_row::FromRow<'r>,
        A: IntoArguments,
    {
        query_as::query_statement_as_with(self, arguments)
    }

    pub fn query_scalar<O>(&self) -> query_scalar::QueryScalar<O, Arguments>
    where
        (O,): for<'r> from_row::FromRow<'r>,
    {
        query_scalar::query_statement_scalar(self)
    }

    pub fn query_scalar_with<'s, O, A>(&'s self, arguments: A) -> query_scalar::QueryScalar<O, A>
    where
        (O,): for<'r> from_row::FromRow<'r>,
        A: IntoArguments,
    {
        query_scalar::query_statement_scalar_with(self, arguments)
    }
}
