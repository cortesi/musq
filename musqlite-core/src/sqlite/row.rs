#![allow(clippy::rc_buffer)]

use std::sync::Arc;

use crate::{
    column::ColumnIndex,
    decode::Decode,
    error::{mismatched_types, Error},
    ext::ustr::UStr,
    sqlite::{statement::StatementHandle, Column, Value, ValueRef},
    types::Type,
    HashMap,
};

/// Implementation of [`Row`] for SQLite.
pub struct Row {
    pub(crate) values: Box<[Value]>,
    pub(crate) columns: Arc<Vec<Column>>,
    pub(crate) column_names: Arc<HashMap<UStr, usize>>,
}

impl ColumnIndex<Row> for usize {
    fn index(&self, row: &Row) -> Result<usize, Error> {
        let len = Row::len(row);

        if *self >= len {
            return Err(Error::ColumnIndexOutOfBounds { len, index: *self });
        }

        Ok(*self)
    }
}

// Accessing values from the statement object is
// safe across threads as long as we don't call [sqlite3_step]

// we block ourselves from doing that by only exposing
// a set interface on [StatementHandle]

unsafe impl Send for Row {}
unsafe impl Sync for Row {}

impl Row {
    pub(crate) fn current(
        statement: &StatementHandle,
        columns: &Arc<Vec<Column>>,
        column_names: &Arc<HashMap<UStr, usize>>,
    ) -> Self {
        let size = statement.column_count();
        let mut values = Vec::with_capacity(size);

        for i in 0..size {
            values.push(unsafe {
                let raw = statement.column_value(i);

                Value::new(raw, columns[i].type_info.clone())
            });
        }

        Self {
            values: values.into_boxed_slice(),
            columns: Arc::clone(columns),
            column_names: Arc::clone(column_names),
        }
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn try_get_raw<I>(&self, index: I) -> Result<ValueRef<'_>, Error>
    where
        I: ColumnIndex<Self>,
    {
        let index = index.index(self)?;
        Ok(ValueRef::value(&self.values[index]))
    }

    /// Returns `true` if this row has no columns.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of columns in this row.
    #[inline]
    pub fn len(&self) -> usize {
        self.columns().len()
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

    /// Index into the database row and decode a single value.
    ///
    /// A string index can be used to access a column by name and a `usize` index
    /// can be used to access a column by position.
    ///
    /// # Panics
    ///
    /// Panics if the column does not exist or its value cannot be decoded into the requested type.
    /// See [`try_get`](Self::try_get) for a non-panicking version.
    ///
    #[inline]
    pub fn get<'r, T, I>(&'r self, index: I) -> T
    where
        I: ColumnIndex<Self>,
        T: Decode<'r> + Type,
    {
        self.try_get::<T, I>(index).unwrap()
    }

    /// Index into the database row and decode a single value.
    ///
    /// Unlike [`get`](Self::get), this method does not check that the type
    /// being returned from the database is compatible with the Rust type and blindly tries
    /// to decode the value.
    ///
    /// # Panics
    ///
    /// Panics if the column does not exist or its value cannot be decoded into the requested type.
    /// See [`try_get_unchecked`](Self::try_get_unchecked) for a non-panicking version.
    ///
    #[inline]
    pub fn get_unchecked<'r, T, I>(&'r self, index: I) -> T
    where
        I: ColumnIndex<Self>,
        T: Decode<'r>,
    {
        self.try_get_unchecked::<T, I>(index).unwrap()
    }

    /// Index into the database row and decode a single value.
    ///
    /// A string index can be used to access a column by name and a `usize` index
    /// can be used to access a column by position.
    ///
    /// # Errors
    ///
    ///  * [`ColumnNotFound`] if the column by the given name was not found.
    ///  * [`ColumnIndexOutOfBounds`] if the `usize` index was greater than the number of columns in the row.
    ///  * [`ColumnDecode`] if the value could not be decoded into the requested type.
    ///
    /// [`ColumnDecode`]: Error::ColumnDecode
    /// [`ColumnNotFound`]: Error::ColumnNotFound
    /// [`ColumnIndexOutOfBounds`]: Error::ColumnIndexOutOfBounds
    ///
    pub fn try_get<'r, T, I>(&'r self, index: I) -> Result<T, Error>
    where
        I: ColumnIndex<Self>,
        T: Decode<'r> + Type,
    {
        let value = self.try_get_raw(&index)?;

        if !value.is_null() {
            let ty = value.type_info();

            if !ty.is_null() && !T::compatible(&ty) {
                return Err(Error::ColumnDecode {
                    index: format!("{:?}", index),
                    source: mismatched_types::<T>(&ty),
                });
            }
        }

        T::decode(value).map_err(|source| Error::ColumnDecode {
            index: format!("{:?}", index),
            source,
        })
    }

    /// Index into the database row and decode a single value.
    ///
    /// Unlike [`try_get`](Self::try_get), this method does not check that the type
    /// being returned from the database is compatible with the Rust type and blindly tries
    /// to decode the value.
    ///
    /// # Errors
    ///
    ///  * [`ColumnNotFound`] if the column by the given name was not found.
    ///  * [`ColumnIndexOutOfBounds`] if the `usize` index was greater than the number of columns in the row.
    ///  * [`ColumnDecode`] if the value could not be decoded into the requested type.
    ///
    /// [`ColumnDecode`]: Error::ColumnDecode
    /// [`ColumnNotFound`]: Error::ColumnNotFound
    /// [`ColumnIndexOutOfBounds`]: Error::ColumnIndexOutOfBounds
    ///
    #[inline]
    pub fn try_get_unchecked<'r, T, I>(&'r self, index: I) -> Result<T, Error>
    where
        I: ColumnIndex<Self>,
        T: Decode<'r>,
    {
        let value = self.try_get_raw(&index)?;

        T::decode(value).map_err(|source| Error::ColumnDecode {
            index: format!("{:?}", index),
            source,
        })
    }
}

impl ColumnIndex<Row> for &'_ str {
    fn index(&self, row: &Row) -> Result<usize, Error> {
        row.column_names
            .get(*self)
            .ok_or_else(|| Error::ColumnNotFound((*self).into()))
            .map(|v| *v)
    }
}

// #[cfg(feature = "any")]
// impl From<SqliteRow> for crate::sqlite::any::AnyRow {
//     #[inline]
//     fn from(row: SqliteRow) -> Self {
//         crate::sqlite::any::AnyRow {
//             columns: row.columns.iter().map(|col| col.clone().into()).collect(),
//             kind: crate::sqlite::any::row::AnyRowKind::Sqlite(row),
//         }
//     }
// }
