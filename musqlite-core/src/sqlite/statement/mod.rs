use crate::{
    ext::ustr::UStr,
    impl_statement_query,
    sqlite::{column::ColumnIndex, error::Error, Arguments, Column, TypeInfo},
    Either, HashMap,
};

use std::borrow::Cow;
use std::sync::Arc;

mod handle;
pub(super) mod unlock_notify;
mod r#virtual;

pub(crate) use handle::StatementHandle;
pub(crate) use r#virtual::VirtualStatement;

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
pub struct Statement<'q> {
    pub(crate) sql: Cow<'q, str>,
    pub(crate) parameters: usize,
    pub(crate) columns: Arc<Vec<Column>>,
    pub(crate) column_names: Arc<HashMap<UStr, usize>>,
}

impl ColumnIndex<Statement<'_>> for usize {
    fn index(&self, statement: &Statement<'_>) -> Result<usize, Error> {
        let len = Statement::columns(statement).len();

        if *self >= len {
            return Err(Error::ColumnIndexOutOfBounds { len, index: *self });
        }

        Ok(*self)
    }
}

impl<'q> Statement<'q> {
    pub fn to_owned(&self) -> Statement<'static> {
        Statement::<'static> {
            sql: Cow::Owned(self.sql.clone().into_owned()),
            parameters: self.parameters,
            columns: Arc::clone(&self.columns),
            column_names: Arc::clone(&self.column_names),
        }
    }

    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn parameters(&self) -> Option<Either<&[TypeInfo], usize>> {
        Some(Either::Right(self.parameters))
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Gets the column information at `index`.
    ///
    /// A string index can be used to access a column by name and a `usize` index
    /// can be used to access a column by position.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    /// See [`try_column`](Self::try_column) for a non-panicking version.
    pub fn column<I>(&self, index: I) -> &Column
    where
        I: ColumnIndex<Self>,
    {
        self.try_column(index).unwrap()
    }

    /// Gets the column information at `index` or `None` if out of bounds.
    pub fn try_column<I>(&self, index: I) -> Result<&Column, Error>
    where
        I: ColumnIndex<Self>,
    {
        Ok(&self.columns()[index.index(self)?])
    }

    impl_statement_query!(Arguments<'_>);
}

impl ColumnIndex<Statement<'_>> for &'_ str {
    fn index(&self, statement: &Statement<'_>) -> Result<usize, Error> {
        statement
            .column_names
            .get(*self)
            .ok_or_else(|| Error::ColumnNotFound((*self).into()))
            .map(|v| *v)
    }
}

// #[cfg(feature = "any")]
// impl<'q> From<SqliteStatement<'q>> for crate::sqlite::any::AnyStatement<'q> {
//     #[inline]
//     fn from(statement: SqliteStatement<'q>) -> Self {
//         crate::sqlite::any::AnyStatement::<'q> {
//             columns: statement
//                 .columns
//                 .iter()
//                 .map(|col| col.clone().into())
//                 .collect(),
//             column_names: statement.column_names,
//             parameters: Some(Either::Right(statement.parameters)),
//             sql: statement.sql,
//         }
//     }
// }
