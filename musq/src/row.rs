#![allow(clippy::rc_buffer)]

use std::sync::Arc;

use crate::{
    decode::Decode,
    error::Error,
    sqlite::{statement::StatementHandle, Value},
    types::Type,
    ustr::UStr,
    Column, HashMap, Result,
};

/// Implementation of [`Row`] for SQLite.
pub struct Row {
    pub values: Box<[Value]>,
    pub columns: Arc<Vec<Column>>,
    pub(crate) column_names: Arc<HashMap<UStr, usize>>,
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

                Value::new(raw, columns[i].type_info)
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

    /// Returns `true` if this row has no columns.

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of columns in this row.

    pub fn len(&self) -> usize {
        self.columns().len()
    }

    /// Get a single value from the row by column index.
    pub fn get_value_idx<'r, T>(&'r self, index: usize) -> Result<T>
    where
        T: Decode<'r> + Type,
    {
        let value = if let Some(v) = self.values.get(index) {
            v
        } else {
            return Err(Error::ColumnIndexOutOfBounds {
                index,
                len: self.values.len(),
            });
        };

        if !value.is_null() {
            let ty = value.type_info();

            if !ty.is_null() && !T::compatible(&ty) {
                return Err(Error::ColumnDecode {
                    index: format!("{:?}", index),
                    source: format!(
                        "mismatched types; Rust type `{}` (as SQLite type `{}`) is not compatible with SQLite type `{}`",
                        ty.name(),
                        T::type_info().name(),
                        ty.name()
                    )
                    .into()
                });
            }
        }

        T::decode(value).map_err(|source| Error::ColumnDecode {
            index: format!("{:?}", index),
            source,
        })
    }

    /// Get a single value from the row by column name.
    pub fn get_value<'r, T>(&'r self, column: &str) -> Result<T>
    where
        T: Decode<'r> + Type,
    {
        self.get_value_idx(
            *self
                .column_names
                .get(column)
                .ok_or_else(|| Error::ColumnNotFound(column.into()))?,
        )
    }
}
