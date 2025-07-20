use std::{collections::HashMap, sync::Arc};

use crate::{
    Column, Result,
    decode::Decode,
    error::Error,
    sqlite::{Value, statement::StatementHandle},
    ustr::UStr,
};

/// Implementation of [`Row`] for SQLite.
#[derive(Clone)]
pub struct Row {
    values: Box<[Value]>,
    columns: Arc<Vec<Column>>,
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
        use crate::sqlite::value::ValueData;
        use libsqlite3_sys::SQLITE_NULL;

        let size = statement.column_count();
        let mut values = Vec::with_capacity(size);

        for i in 0..size {
            let code = statement.column_type(i);
            let data = match code {
                SQLITE_NULL => ValueData::Null,
                libsqlite3_sys::SQLITE_INTEGER => ValueData::Integer(statement.column_int64(i)),
                libsqlite3_sys::SQLITE_FLOAT => ValueData::Double(statement.column_double(i)),
                libsqlite3_sys::SQLITE_TEXT => {
                    let len = statement.column_bytes(i) as usize;
                    let ptr = statement.column_blob(i) as *const u8;
                    let slice = if len == 0 {
                        &[]
                    } else {
                        unsafe { std::slice::from_raw_parts(ptr, len) }
                    };
                    ValueData::Text(slice.to_vec())
                }
                libsqlite3_sys::SQLITE_BLOB => {
                    let len = statement.column_bytes(i) as usize;
                    if len == 0 {
                        ValueData::Blob(Vec::new())
                    } else {
                        let ptr = statement.column_blob(i) as *const u8;
                        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
                        ValueData::Blob(slice.to_vec())
                    }
                }
                _ => unreachable!(),
            };

            values.push(Value {
                data,
                type_info: columns[i].type_info,
            });
        }

        Self {
            values: values.into_boxed_slice(),
            columns: Arc::clone(columns),
            column_names: Arc::clone(column_names),
        }
    }

    /// Returns the values for this row.
    pub fn values(&self) -> &[Value] {
        self.values.as_ref()
    }

    /// Returns the column definitions for this row.
    pub fn columns(&self) -> &[Column] {
        self.columns.as_ref()
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

impl<'r> crate::from_row::FromRow<'r> for Row {
    fn from_row(_prefix: &str, row: &'r Row) -> Result<Self> {
        Ok(row.clone())
    }
}
