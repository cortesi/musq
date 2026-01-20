use std::{collections::HashMap, ops::Range, slice, str, sync::Arc};

use bytes::BytesMut;

use crate::{
    Result,
    column::Column,
    decode::Decode,
    error::{DecodeError, Error},
    from_row::FromRow,
    sqlite::{SqliteDataType, Value, statement::StatementHandle},
};

/// Implementation of [`Row`] for SQLite.
#[derive(Clone)]
pub struct Row {
    /// Values for each column in the row.
    values: Box<[Value]>,
    /// Column metadata.
    columns: Arc<Vec<Column>>,
    /// Column name lookup table.
    pub(crate) column_names: Arc<HashMap<Arc<str>, usize>>,
}

// Accessing values from the statement object is
// safe across threads as long as we don't call [sqlite3_step]

// we block ourselves from doing that by only exposing
// a set interface on [StatementHandle]

unsafe impl Send for Row {}
unsafe impl Sync for Row {}

impl Row {
    /// Build a row from the current statement position.
    pub(crate) fn current(
        statement: &StatementHandle,
        columns: &Arc<Vec<Column>>,
        column_names: &Arc<HashMap<Arc<str>, usize>>,
    ) -> Result<Self> {
        use libsqlite3_sys::{SQLITE_BLOB, SQLITE_NULL, SQLITE_TEXT};

        /// Variable-length column variant that will be materialized after the shared buffer is built.
        enum DeferredKind {
            Text,
            Blob,
        }

        let size = statement.column_count();
        let mut variable_bytes = 0usize;
        for i in 0..size {
            match statement.column_type(i) {
                SQLITE_TEXT => {
                    // Ensure TEXT has been converted into UTF-8 so `column_bytes` matches the
                    // `column_text` representation.
                    statement.column_text(i);
                    variable_bytes += statement.column_bytes(i) as usize;
                }
                SQLITE_BLOB => variable_bytes += statement.column_bytes(i) as usize,
                _ => {}
            }
        }

        let mut values = Vec::with_capacity(size);
        let mut buffer = BytesMut::with_capacity(variable_bytes);
        let mut deferred: Vec<(usize, DeferredKind, Range<usize>, Option<SqliteDataType>)> =
            Vec::new();

        for i in 0..size {
            let declared_type = columns[i].type_info;
            let type_info = (declared_type != SqliteDataType::Null).then_some(declared_type);

            let code = statement.column_type(i);
            match code {
                SQLITE_NULL => values.push(Value::Null { type_info }),
                libsqlite3_sys::SQLITE_INTEGER => values.push(Value::Integer {
                    value: statement.column_int64(i),
                    type_info,
                }),
                libsqlite3_sys::SQLITE_FLOAT => values.push(Value::Double {
                    value: statement.column_double(i),
                    type_info,
                }),
                SQLITE_TEXT => {
                    let ptr = statement.column_text(i);
                    let len = statement.column_bytes(i) as usize;
                    let slice = if len == 0 {
                        &[]
                    } else if ptr.is_null() {
                        return Err(Error::Protocol("sqlite3_column_text returned null".into()));
                    } else {
                        unsafe { slice::from_raw_parts(ptr, len) }
                    };

                    str::from_utf8(slice).map_err(|e| {
                        Error::Decode(DecodeError::Conversion(format!(
                            "invalid UTF-8 in TEXT column {i}: {e}"
                        )))
                    })?;

                    let start = buffer.len();
                    buffer.extend_from_slice(slice);
                    let end = buffer.len();

                    let idx = values.len();
                    values.push(Value::Null { type_info: None });
                    deferred.push((idx, DeferredKind::Text, start..end, type_info));
                }
                SQLITE_BLOB => {
                    let len = statement.column_bytes(i) as usize;
                    let slice = if len == 0 {
                        &[]
                    } else {
                        let ptr = statement.column_blob(i) as *const u8;
                        unsafe { slice::from_raw_parts(ptr, len) }
                    };

                    let start = buffer.len();
                    buffer.extend_from_slice(slice);
                    let end = buffer.len();

                    let idx = values.len();
                    values.push(Value::Null { type_info: None });
                    deferred.push((idx, DeferredKind::Blob, start..end, type_info));
                }
                _ => return Err(Error::UnknownColumnType(code)),
            }
        }

        let shared = buffer.freeze();
        for (idx, kind, range, type_info) in deferred {
            let value = shared.slice(range);
            values[idx] = match kind {
                DeferredKind::Text => Value::Text { value, type_info },
                DeferredKind::Blob => Value::Blob { value, type_info },
            };
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

        T::decode(value).map_err(|source| {
            let column_name = self
                .column_names
                .iter()
                .find_map(|(name, &idx)| if idx == index { Some(&**name) } else { None })
                .unwrap_or("unknown")
                .to_string();

            Error::ColumnDecode {
                index: format!("{index:?}"),
                column_name,
                value: value.clone(),
                source,
            }
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

impl<'r> FromRow<'r> for Row {
    fn from_row(_prefix: &str, row: &'r Row) -> Result<Self> {
        Ok(row.clone())
    }
}
