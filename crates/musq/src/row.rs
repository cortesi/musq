use std::{collections::HashMap, sync::Arc};

use crate::{
    Column, Result,
    decode::Decode,
    error::Error,
    sqlite::{SqliteDataType, Value, statement::StatementHandle},
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
    ) -> Result<Self> {
        use crate::sqlite::Value;
        use libsqlite3_sys::SQLITE_NULL;

        let size = statement.column_count();
        let mut values = Vec::with_capacity(size);

        for i in 0..size {
            let code = statement.column_type(i);
            let val = match code {
                SQLITE_NULL => Value::Null {
                    type_info: if columns[i].type_info == SqliteDataType::Null {
                        None
                    } else {
                        Some(columns[i].type_info)
                    },
                },
                libsqlite3_sys::SQLITE_INTEGER => Value::Integer {
                    value: statement.column_int64(i),
                    type_info: if columns[i].type_info == SqliteDataType::Null {
                        None
                    } else {
                        Some(columns[i].type_info)
                    },
                },
                libsqlite3_sys::SQLITE_FLOAT => Value::Double {
                    value: statement.column_double(i),
                    type_info: if columns[i].type_info == SqliteDataType::Null {
                        None
                    } else {
                        Some(columns[i].type_info)
                    },
                },
                libsqlite3_sys::SQLITE_TEXT => {
                    let len = statement.column_bytes(i) as usize;
                    let ptr = statement.column_blob(i) as *const u8;
                    let slice = if len == 0 {
                        &[]
                    } else {
                        unsafe { std::slice::from_raw_parts(ptr, len) }
                    };
                    Value::Text {
                        value: slice.to_vec(),
                        type_info: if columns[i].type_info == SqliteDataType::Null {
                            None
                        } else {
                            Some(columns[i].type_info)
                        },
                    }
                }
                libsqlite3_sys::SQLITE_BLOB => {
                    let len = statement.column_bytes(i) as usize;
                    let vec = if len == 0 {
                        Vec::new()
                    } else {
                        let ptr = statement.column_blob(i) as *const u8;
                        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
                        slice.to_vec()
                    };
                    Value::Blob {
                        value: vec,
                        type_info: if columns[i].type_info == SqliteDataType::Null {
                            None
                        } else {
                            Some(columns[i].type_info)
                        },
                    }
                }
                _ => return Err(Error::UnknownColumnType(code)),
            };

            values.push(val);
        }

        Ok(Self {
            values: values.into_boxed_slice(),
            columns: Arc::clone(columns),
            column_names: Arc::clone(column_names),
        })
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
