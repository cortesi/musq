use std::{
    ffi::{CStr, c_void},
    os::raw::c_char,
    ptr::NonNull,
    result::Result as StdResult,
};

use libsqlite3_sys::{SQLITE_ROW, sqlite3, sqlite3_stmt};

use crate::sqlite::{error::SqliteError, ffi, type_info::SqliteDataType};

/// Wrapper around a raw SQLite statement handle.
#[derive(Debug)]
pub struct StatementHandle(NonNull<sqlite3_stmt>);

// access to SQLite3 statement handles are safe to send and share between
// threads as long as the `sqlite3_step` call is serialized.

unsafe impl Send for StatementHandle {}

impl StatementHandle {
    /// Create a new statement handle wrapper.
    pub(super) fn new(ptr: NonNull<sqlite3_stmt>) -> Self {
        Self(ptr)
    }

    /// Return the underlying SQLite database handle for this statement.
    pub(super) unsafe fn db_handle(&self) -> *mut sqlite3 {
        // O(c) access to the connection handle for this statement handle
        // https://sqlite.org/c3ref/db_handle.html
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::db_handle(self.0.as_ptr()) }
    }

    /// Return the last SQLite error for this statement.
    pub(crate) fn last_error(&self) -> SqliteError {
        // SAFETY: this handle owns a live prepared statement.
        SqliteError::new(unsafe { self.db_handle() })
    }

    /// Return the number of columns in the result set.
    pub(crate) fn column_count(&self) -> usize {
        // https://sqlite.org/c3ref/column_count.html
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::column_count(self.0.as_ptr()) as usize }
    }

    /// Return the number of changes from the last statement.
    pub(crate) fn changes(&self) -> u64 {
        // returns the number of changes of the *last* statement; not
        // necessarily this statement.
        // https://sqlite.org/c3ref/changes.html
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::changes64(self.db_handle()) as u64 }
    }

    /// Returns `true` if this statement is read-only.
    pub(crate) fn is_readonly(&self) -> bool {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::stmt_readonly(self.0.as_ptr()) }
    }

    /// Return the name of a result column.
    pub(crate) fn column_name(&self, index: usize) -> StdResult<String, SqliteError> {
        // https://sqlite.org/c3ref/column_name.html
        // SAFETY: this handle owns a live prepared statement.
        let name = unsafe { ffi::column_name(self.0.as_ptr(), index as i32) };
        if name.is_null() {
            return Err(self.last_error());
        }

        let s = unsafe { CStr::from_ptr(name) };
        Ok(s.to_string_lossy().into_owned())
    }

    /// Return the type information for a result column.
    pub(crate) fn column_type_info(&self, index: usize) -> Option<SqliteDataType> {
        SqliteDataType::from_code(self.column_type(index))
    }

    /// Return the declared type for a result column, if available.
    pub(crate) fn column_decltype(&self, index: usize) -> Option<SqliteDataType> {
        // SAFETY: this handle owns a live prepared statement.
        let decl = unsafe { ffi::column_decltype(self.0.as_ptr(), index as i32) };
        if decl.is_null() {
            // If the Nth column of the result set is an expression or subquery,
            // then a NULL pointer is returned.
            return None;
        }

        let decl = unsafe { CStr::from_ptr(decl).to_string_lossy() };
        SqliteDataType::from_str(&decl)
    }

    // Number Of SQL Parameters

    /// Return the number of bind parameters.
    pub(crate) fn bind_parameter_count(&self) -> usize {
        // https://www.sqlite.org/c3ref/bind_parameter_count.html
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::bind_parameter_count(self.0.as_ptr()) as usize }
    }

    // Name Of A Host Parameter
    // NOTE: The first host parameter has an index of 1, not 0.

    /// Return the name of a bind parameter, if any.
    pub(crate) fn bind_parameter_name(&self, index: usize) -> Option<String> {
        // https://www.sqlite.org/c3ref/bind_parameter_name.html
        // SAFETY: this handle owns a live prepared statement.
        let name = unsafe { ffi::bind_parameter_name(self.0.as_ptr(), index as i32) };
        if name.is_null() {
            return None;
        }

        let s = unsafe { CStr::from_ptr(name) };
        Some(s.to_string_lossy().into_owned())
    }

    // Binding Values To Prepared Statements
    // https://www.sqlite.org/c3ref/bind_blob.html

    /// Bind a blob parameter.
    pub(crate) fn bind_blob(&self, index: usize, v: &[u8]) -> StdResult<(), SqliteError> {
        // SAFETY: this handle owns a live prepared statement.
        unsafe {
            ffi::bind_blob64(
                self.0.as_ptr(),
                index as i32,
                v.as_ptr() as *const c_void,
                v.len() as u64,
            )
        }
    }

    /// Bind a text parameter.
    pub(crate) fn bind_text(&self, index: usize, v: &str) -> StdResult<(), SqliteError> {
        // SAFETY: this handle owns a live prepared statement.
        unsafe {
            ffi::bind_text64(
                self.0.as_ptr(),
                index as i32,
                v.as_ptr() as *const c_char,
                v.len() as u64,
            )
        }
    }

    /// Bind a 64-bit integer parameter.
    pub(crate) fn bind_int64(&self, index: usize, v: i64) -> StdResult<(), SqliteError> {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::bind_int64(self.0.as_ptr(), index as i32, v) }
    }

    /// Bind a floating-point parameter.
    pub(crate) fn bind_double(&self, index: usize, v: f64) -> StdResult<(), SqliteError> {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::bind_double(self.0.as_ptr(), index as i32, v) }
    }

    /// Bind a NULL parameter.
    pub(crate) fn bind_null(&self, index: usize) -> StdResult<(), SqliteError> {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::bind_null(self.0.as_ptr(), index as i32) }
    }

    // result values from the query
    // https://www.sqlite.org/c3ref/column_blob.html

    /// Return the SQLite type code for a result column.
    pub(crate) fn column_type(&self, index: usize) -> i32 {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::column_type(self.0.as_ptr(), index as i32) }
    }

    /// Return an integer value from a result column.
    pub(crate) fn column_int64(&self, index: usize) -> i64 {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::column_int64(self.0.as_ptr(), index as i32) }
    }

    /// Return a floating-point value from a result column.
    pub(crate) fn column_double(&self, index: usize) -> f64 {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::column_double(self.0.as_ptr(), index as i32) }
    }

    /// Return a text pointer from a result column.
    pub(crate) fn column_text(&self, index: usize) -> *const u8 {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::column_text(self.0.as_ptr(), index as i32) }
    }

    /// Return a blob pointer from a result column.
    pub(crate) fn column_blob(&self, index: usize) -> *const c_void {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::column_blob(self.0.as_ptr(), index as i32) }
    }

    /// Return the number of bytes in a result column.
    pub(crate) fn column_bytes(&self, index: usize) -> i32 {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::column_bytes(self.0.as_ptr(), index as i32) }
    }

    /// Clear all bound parameters.
    pub(crate) fn clear_bindings(&self) {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::clear_bindings(self.0.as_ptr()) }
    }

    /// Reset the statement so it can be re-executed.
    pub(crate) fn reset(&self) -> StdResult<(), SqliteError> {
        // SAFETY: this handle owns a live prepared statement.
        unsafe { ffi::reset(self.0.as_ptr())? }

        Ok(())
    }

    /// Step the statement, returning whether a row is available.
    pub(crate) fn step(&self) -> crate::Result<bool> {
        // SAFETY: this handle owns a live prepared statement.
        Ok(unsafe { ffi::step(self.0.as_ptr()) }.map_err(crate::Error::from)? == SQLITE_ROW)
    }
}

impl Drop for StatementHandle {
    fn drop(&mut self) {
        // SAFETY: we have exclusive access to the `StatementHandle` here
        {
            // Ensure the statement is reset before finalizing so that
            // sqlite3_finalize does not return SQLITE_BUSY.
            // SAFETY: this handle still owns the statement until finalize returns.
            if let Err(e) = unsafe { ffi::reset(self.0.as_ptr()) } {
                tracing::error!("sqlite3_reset before finalize failed: {}", e);
            }

            // https://sqlite.org/c3ref/finalize.html
            // Never touch the statement pointer after finalize, and never panic
            // in Drop: a panic during unwind aborts the process.
            if let Err(e) = unsafe { ffi::finalize(self.0.as_ptr()) } {
                tracing::error!("sqlite3_finalize failed: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::CString,
        ptr::{self, NonNull},
    };

    use super::*;
    use crate::sqlite::error::PrimaryErrCode;

    fn open_memory() -> *mut sqlite3 {
        let filename = CString::new(":memory:").unwrap();
        let mut handle = ptr::null_mut();
        // SAFETY: this test owns the connection pointer it opens.
        unsafe {
            ffi::open_v2(
                filename.as_ptr(),
                &mut handle,
                libsqlite3_sys::SQLITE_OPEN_READWRITE
                    | libsqlite3_sys::SQLITE_OPEN_CREATE
                    | libsqlite3_sys::SQLITE_OPEN_MEMORY,
                ptr::null(),
            )
        }
        .unwrap();
        handle
    }

    fn prepare_failing_insert(db: *mut sqlite3) -> NonNull<sqlite3_stmt> {
        let create_sql = CString::new("CREATE TABLE t (id INTEGER PRIMARY KEY);").unwrap();
        // SAFETY: `db` is a live connection opened by this test.
        unsafe { ffi::exec(db, create_sql.as_ptr()) }.unwrap();
        let insert_sql = CString::new("INSERT INTO t VALUES (1);").unwrap();
        unsafe { ffi::exec(db, insert_sql.as_ptr()) }.unwrap();

        let dup_sql = CString::new("INSERT INTO t VALUES (1);").unwrap();
        let mut stmt = ptr::null_mut();
        unsafe { ffi::prepare_v3(db, dup_sql.as_ptr(), -1, 0, &mut stmt, ptr::null_mut()) }
            .unwrap();
        let err = unsafe { ffi::step(stmt) }.expect_err("duplicate insert must fail");
        assert_eq!(err.primary, PrimaryErrCode::Constraint);
        NonNull::new(stmt).expect("prepared statement pointer")
    }

    #[test]
    fn drop_after_failed_step_does_not_panic() {
        let db = open_memory();
        let stmt = prepare_failing_insert(db);
        drop(StatementHandle::new(stmt));
        unsafe { ffi::close(db) }.unwrap();
    }
}
