use crate::{error::Error, ext::ustr::UStr, sqlite::TypeInfo};

use std::fmt::Debug;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Column {
    pub(crate) name: UStr,
    pub(crate) ordinal: usize,
    pub(crate) type_info: TypeInfo,
}

impl Column {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn name(&self) -> &str {
        &*self.name
    }

    pub fn type_info(&self) -> &TypeInfo {
        &self.type_info
    }
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
