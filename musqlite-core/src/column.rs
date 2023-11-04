use crate::{error::Error, sqlite};

use std::fmt::Debug;

pub trait Column: 'static + Send + Sync + Debug {
    /// Gets the column ordinal.
    ///
    /// This can be used to unambiguously refer to this column within a row in case more than
    /// one column have the same name
    fn ordinal(&self) -> usize;

    /// Gets the column name or alias.
    ///
    /// The column name is unreliable (and can change between database minor versions) if this
    /// column is an expression that has not been aliased.
    fn name(&self) -> &str;

    /// Gets the type information for the column.
    fn type_info(&self) -> &sqlite::TypeInfo;
}

/// A type that can be used to index into a [`Row`] or [`Statement`].
///
/// The [`get`] and [`try_get`] methods of [`Row`] accept any type that implements `ColumnIndex`.
/// This trait is implemented for strings which are used to look up a column by name, and for
/// `usize` which is used as a positional index into the row.
///
/// This trait is sealed and cannot be implemented for types outside of SQLx.
///
/// [`Row`]: crate::row::Row
/// [`Statement`]: crate::statement::Statement
/// [`get`]: crate::row::Row::get
/// [`try_get`]: crate::row::Row::try_get
///
pub trait ColumnIndex<T: ?Sized>: Debug {
    /// Returns a valid positional index into the row or statement, [`ColumnIndexOutOfBounds`], or,
    /// [`ColumnNotFound`].
    ///
    /// [`ColumnNotFound`]: Error::ColumnNotFound
    /// [`ColumnIndexOutOfBounds`]: Error::ColumnIndexOutOfBounds
    fn index(&self, container: &T) -> Result<usize, Error>;
}

impl<T: ?Sized, I: ColumnIndex<T> + ?Sized> ColumnIndex<T> for &'_ I {
    #[inline]
    fn index(&self, row: &T) -> Result<usize, Error> {
        (**self).index(row)
    }
}
