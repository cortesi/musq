use std::{
    ffi::{CStr, c_void},
    os::raw::c_char,
    ptr::NonNull,
    result::Result as StdResult,
};

use libsqlite3_sys::{
    SQLITE_DONE, SQLITE_LOCKED_SHAREDCACHE, SQLITE_MISUSE, SQLITE_ROW, sqlite3, sqlite3_stmt,
};

use super::unlock_notify;
use crate::sqlite::{
    DEFAULT_MAX_RETRIES,
    error::{PrimaryErrCode, SqliteError},
    ffi,
    type_info::SqliteDataType,
};

/// Wrapper around a raw SQLite statement handle.
#[derive(Debug)]
pub struct StatementHandle(NonNull<sqlite3_stmt>);

// access to SQLite3 statement handles are safe to send and share between threads
// as long as the `sqlite3_step` call is serialized.

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
        ffi::db_handle(self.0.as_ptr())
    }

    /// Return the last SQLite error for this statement.
    pub(crate) fn last_error(&self) -> SqliteError {
        SqliteError::new(unsafe { self.db_handle() })
    }

    /// Return the number of columns in the result set.
    pub(crate) fn column_count(&self) -> usize {
        // https://sqlite.org/c3ref/column_count.html
        ffi::column_count(self.0.as_ptr()) as usize
    }

    /// Return the number of changes from the last statement.
    pub(crate) fn changes(&self) -> u64 {
        // returns the number of changes of the *last* statement; not
        // necessarily this statement.
        // https://sqlite.org/c3ref/changes.html
        unsafe { ffi::changes(self.db_handle()) as u64 }
    }

    /// Return the name of a result column.
    pub(crate) fn column_name(&self, index: usize) -> StdResult<String, SqliteError> {
        // https://sqlite.org/c3ref/column_name.html
        let name = ffi::column_name(self.0.as_ptr(), index as i32);
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
        let decl = ffi::column_decltype(self.0.as_ptr(), index as i32);
        if decl.is_null() {
            // If the Nth column of the result set is an expression or subquery,
            // then a NULL pointer is returned.
            return None;
        }

        let decl = unsafe { CStr::from_ptr(decl).to_string_lossy() };
        let ty: SqliteDataType = decl.parse().ok()?;

        Some(ty)
    }

    // Number Of SQL Parameters

    /// Return the number of bind parameters.
    pub(crate) fn bind_parameter_count(&self) -> usize {
        // https://www.sqlite.org/c3ref/bind_parameter_count.html
        ffi::bind_parameter_count(self.0.as_ptr()) as usize
    }

    // Name Of A Host Parameter
    // NOTE: The first host parameter has an index of 1, not 0.

    /// Return the name of a bind parameter, if any.
    pub(crate) fn bind_parameter_name(&self, index: usize) -> Option<String> {
        // https://www.sqlite.org/c3ref/bind_parameter_name.html
        let name = ffi::bind_parameter_name(self.0.as_ptr(), index as i32);
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
        ffi::bind_blob64(
            self.0.as_ptr(),
            index as i32,
            v.as_ptr() as *const c_void,
            v.len() as u64,
        )
    }

    /// Bind a text parameter.
    pub(crate) fn bind_text(&self, index: usize, v: &str) -> StdResult<(), SqliteError> {
        ffi::bind_text64(
            self.0.as_ptr(),
            index as i32,
            v.as_ptr() as *const c_char,
            v.len() as u64,
        )
    }

    /// Bind a 64-bit integer parameter.
    pub(crate) fn bind_int64(&self, index: usize, v: i64) -> StdResult<(), SqliteError> {
        ffi::bind_int64(self.0.as_ptr(), index as i32, v)
    }

    /// Bind a floating-point parameter.
    pub(crate) fn bind_double(&self, index: usize, v: f64) -> StdResult<(), SqliteError> {
        ffi::bind_double(self.0.as_ptr(), index as i32, v)
    }

    /// Bind a NULL parameter.
    pub(crate) fn bind_null(&self, index: usize) -> StdResult<(), SqliteError> {
        ffi::bind_null(self.0.as_ptr(), index as i32)
    }

    // result values from the query
    // https://www.sqlite.org/c3ref/column_blob.html

    /// Return the SQLite type code for a result column.
    pub(crate) fn column_type(&self, index: usize) -> i32 {
        ffi::column_type(self.0.as_ptr(), index as i32)
    }

    /// Return an integer value from a result column.
    pub(crate) fn column_int64(&self, index: usize) -> i64 {
        ffi::column_int64(self.0.as_ptr(), index as i32)
    }

    /// Return a floating-point value from a result column.
    pub(crate) fn column_double(&self, index: usize) -> f64 {
        ffi::column_double(self.0.as_ptr(), index as i32)
    }

    /// Return a blob pointer from a result column.
    pub(crate) fn column_blob(&self, index: usize) -> *const c_void {
        ffi::column_blob(self.0.as_ptr(), index as i32)
    }

    /// Return the number of bytes in a result column.
    pub(crate) fn column_bytes(&self, index: usize) -> i32 {
        ffi::column_bytes(self.0.as_ptr(), index as i32)
    }

    /// Clear all bound parameters.
    pub(crate) fn clear_bindings(&self) {
        ffi::clear_bindings(self.0.as_ptr());
    }

    /// Reset the statement so it can be re-executed.
    pub(crate) fn reset(&self) -> StdResult<(), SqliteError> {
        // SAFETY: we have exclusive access to the handle
        ffi::reset(self.0.as_ptr())?;

        Ok(())
    }

    /// Step the statement, returning whether a row is available.
    pub(crate) fn step(&mut self) -> crate::Result<bool> {
        // SAFETY: we have exclusive access to the handle
        let mut attempts = 0;
        loop {
            let rc = ffi::step(self.0.as_ptr()).map_err(crate::Error::from)?;
            match rc {
                SQLITE_ROW => return Ok(true),
                SQLITE_DONE => return Ok(false),
                SQLITE_MISUSE => {
                    return Err(unsafe { SqliteError::new(self.db_handle()) }.into());
                }
                SQLITE_LOCKED_SHAREDCACHE | libsqlite3_sys::SQLITE_LOCKED => {
                    // The shared cache is locked by another connection. Wait for unlock
                    // notification and try again.
                    if attempts >= DEFAULT_MAX_RETRIES {
                        return Err(crate::Error::UnlockNotify);
                    }
                    attempts += 1;
                    unlock_notify::wait(unsafe { self.db_handle() }, Some(self.0.as_ptr()))?;
                    // Need to reset the handle after the unlock
                    // (https://www.sqlite.org/unlock_notify.html)
                    loop {
                        if attempts >= DEFAULT_MAX_RETRIES {
                            return Err(crate::Error::UnlockNotify);
                        }
                        attempts += 1;
                        match ffi::reset(self.0.as_ptr()) {
                            Ok(()) => break,
                            Err(ref e) if e.should_retry() => {
                                unlock_notify::wait(
                                    unsafe { self.db_handle() },
                                    Some(self.0.as_ptr()),
                                )?;
                                continue;
                            }
                            Err(e) => return Err(e.into()),
                        }
                    }
                }
                libsqlite3_sys::SQLITE_BUSY => {
                    // Another connection holds a lock that prevented the step from
                    // completing. Wait for an unlock notification and retry.
                    if attempts >= DEFAULT_MAX_RETRIES {
                        return Err(crate::Error::UnlockNotify);
                    }
                    attempts += 1;
                    unlock_notify::wait(unsafe { self.db_handle() }, Some(self.0.as_ptr()))?;
                    loop {
                        if attempts >= DEFAULT_MAX_RETRIES {
                            return Err(crate::Error::UnlockNotify);
                        }
                        attempts += 1;
                        match ffi::reset(self.0.as_ptr()) {
                            Ok(()) => break,
                            Err(ref e) if e.should_retry() => {
                                unlock_notify::wait(
                                    unsafe { self.db_handle() },
                                    Some(self.0.as_ptr()),
                                )?;
                                continue;
                            }
                            Err(e) => return Err(e.into()),
                        }
                    }
                }
                _ => return Err(unsafe { SqliteError::new(self.db_handle()) }.into()),
            }
        }
    }
}

impl Drop for StatementHandle {
    fn drop(&mut self) {
        // SAFETY: we have exclusive access to the `StatementHandle` here
        {
            // Ensure the statement is reset before finalizing so that
            // sqlite3_finalize does not return SQLITE_BUSY.
            if let Err(e) = ffi::reset(self.0.as_ptr()) {
                tracing::error!("sqlite3_reset before finalize failed: {}", e);
            }

            // https://sqlite.org/c3ref/finalize.html
            match ffi::finalize(self.0.as_ptr()) {
                Ok(()) => {}
                Err(e) => {
                    if e.primary == PrimaryErrCode::Misuse {
                        panic!("Detected sqlite3_finalize misuse.");
                    } else {
                        tracing::error!(
                            db_ptr = ?unsafe { self.db_handle() },
                            "sqlite3_finalize failed: {}",
                            e
                        );
                    }
                }
            }
        }
    }
}
