use std::{collections::HashMap, sync::Arc};

use crate::{
    Column, Result,
    decode::Decode,
    error::Error,
    sqlite::{Value, statement::StatementHandle},
    ustr::UStr,
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
            let raw = statement.column_value(i);
            values.push(unsafe { Value::new(raw, columns[i].type_info) });
        }

        Self {
            values: values.into_boxed_slice(),
            columns: Arc::clone(columns),
            column_names: Arc::clone(column_names),
        }
    }

    /// Returns `true` if this row has no columns.
    pub fn is_empty(&self) -> bool {
        self.columns.len() == 0
    }

    /// Get a single value from the row by column index.
    pub fn get_value_idx<'r, T>(&'r self, index: usize) -> Result<T>
    where
        T: Decode<'r>,
    {
        let value = if let Some(v) = self.values.get(index) {
            v
        } else {
            return Err(Error::ColumnIndexOutOfBounds {
                index,
                len: self.values.len(),
            });
        };

        T::decode(value).map_err(|source| Error::ColumnDecode {
            index: format!("{index:?}"),
            source,
        })
    }

    /// Get a single value from the row by column name.
    pub fn get_value<'r, T>(&'r self, column: &str) -> Result<T>
    where
        T: Decode<'r>,
    {
        self.get_value_idx(
            *self
                .column_names
                .get(column)
                .ok_or_else(|| Error::ColumnNotFound(column.into()))?,
        )
    }
}
