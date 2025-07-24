use std::{
    cmp,
    collections::HashMap,
    os::raw::c_char,
    ptr::{NonNull, null, null_mut},
    sync::Arc,
};

use crate::sqlite::ffi;
use bytes::{Buf, Bytes};
use libsqlite3_sys::{SQLITE_PREPARE_PERSISTENT, sqlite3, sqlite3_stmt};

use crate::{
    SqliteDataType,
    column::Column,
    error::{Error, Result},
    sqlite::{
        connection::ConnectionHandle,
        statement::{StatementHandle, unlock_notify},
    },
};

// A compound statement consists of *zero* or more raw SQLite3 statements. We chop up a SQL statement
// on `;` to support multiple statements in one query.

#[derive(Debug)]
pub struct CompoundStatement {
    /// the current index of the actual statement that is executing
    /// if `None`, no statement is executing and `prepare()` must be called;
    /// if `Some(self.handles.len())` and `self.tail.is_empty()`,
    /// there are no more statements to execute and `reset()` must be called
    index: Option<usize>,

    /// tail of the most recently prepared SQL statement within this container
    tail: Bytes,

    /// underlying sqlite handles for each inner statement
    /// a SQL query string in SQLite is broken up into N statements
    handles: Vec<StatementHandle>,

    // each set of columns
    columns: Vec<Arc<Vec<Column>>>,

    // each set of column names
    column_names: Vec<Arc<HashMap<Arc<str>, usize>>>,
}

pub struct PreparedStatement<'a> {
    pub(crate) handle: &'a mut StatementHandle,
    pub(crate) columns: &'a Arc<Vec<Column>>,
    pub(crate) column_names: &'a Arc<HashMap<Arc<str>, usize>>,
}

impl CompoundStatement {
    pub(crate) fn new(mut query: &str) -> Result<Self> {
        query = query.trim();

        if query.len() > i32::MAX as usize {
            return Err(Error::Protocol(format!(
                "query string must be smaller than {} bytes",
                i32::MAX
            )));
        }

        Ok(Self {
            tail: Bytes::from(String::from(query)),
            handles: Vec::new(),
            index: None,
            columns: Vec::new(),
            column_names: Vec::new(),
        })
    }

    pub(crate) fn prepare_next(
        &mut self,
        conn: &mut ConnectionHandle,
    ) -> Result<Option<PreparedStatement<'_>>> {
        // increment `self.index` up to `self.handles.len()`
        self.index = self
            .index
            .map(|idx| cmp::min(idx + 1, self.handles.len()))
            .or(Some(0));

        while self.handles.len() <= self.index.unwrap_or(0) {
            if self.tail.is_empty() {
                return Ok(None);
            }

            match prepare_all(conn.as_ptr(), &mut self.tail)? {
                Some(statement) => {
                    let num = statement.column_count();

                    let mut columns = Vec::with_capacity(num);
                    let mut column_names = HashMap::with_capacity(num);

                    for i in 0..num {
                        let name: Arc<str> = statement.column_name(i)?.into();
                        let type_info = statement
                            .column_decltype(i)
                            .or_else(|| statement.column_type_info(i))
                            .unwrap_or(SqliteDataType::Null);

                        columns.push(Column { type_info });

                        column_names.insert(name, i);
                    }

                    self.handles.push(statement);
                    self.columns.push(Arc::new(columns));
                    self.column_names.push(Arc::new(column_names));
                }
                None => {
                    // nothing more to prepare
                    return Ok(None);
                }
            }
        }

        Ok(self.current())
    }

    pub fn current(&mut self) -> Option<PreparedStatement<'_>> {
        self.index
            .filter(|&idx| idx < self.handles.len())
            .map(move |idx| PreparedStatement {
                handle: &mut self.handles[idx],
                columns: &self.columns[idx],
                column_names: &self.column_names[idx],
            })
    }

    pub fn reset(&mut self) -> Result<()> {
        self.index = None;

        let mut first_err: Option<Error> = None;

        for handle in self.handles.iter_mut() {
            if let Err(e) = handle.reset()
                && first_err.is_none()
            {
                first_err = Some(e.into());
            }
            handle.clear_bindings();
        }

        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// Prepare all statements in the given query.
fn prepare_all(conn: *mut sqlite3, query: &mut Bytes) -> Result<Option<StatementHandle>> {
    let flags = SQLITE_PREPARE_PERSISTENT;

    while !query.is_empty() {
        let mut statement_handle: *mut sqlite3_stmt = null_mut();
        let mut tail: *const c_char = null();

        let query_ptr = query.as_ptr() as *const c_char;
        let query_len = query.len() as i32;

        // <https://www.sqlite.org/c3ref/prepare.html>
        loop {
            match ffi::prepare_v3(
                conn,
                query_ptr,
                query_len,
                flags,
                &mut statement_handle,
                &mut tail,
            ) {
                Ok(()) => break,
                Err(e) if e.should_retry() => {
                    unlock_notify::wait(conn, None)?;
                }
                Err(e) => return Err(e.into()),
            }
        }

        // tail should point to the first byte past the end of the first SQL
        // statement in zSql. these routines only compile the first statement,
        // so tail is left pointing to what remains un-compiled.

        let n = (tail as usize) - (query_ptr as usize);

        if n == 0 {
            // SQLite did not consume any bytes of the input. Returning `None`
            // avoids an infinite loop in the caller.
            return Ok(None);
        }

        query.advance(n);

        if let Some(handle) = NonNull::new(statement_handle) {
            return Ok(Some(StatementHandle::new(handle)));
        }
    }

    Ok(None)
}
