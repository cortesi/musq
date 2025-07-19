use std::ffi::CStr;
use std::ffi::c_void;

use std::os::raw::{c_char, c_int};
use std::ptr::NonNull;
use std::str::from_utf8_unchecked;

use libsqlite3_sys::{
    SQLITE_DONE, SQLITE_LOCKED_SHAREDCACHE, SQLITE_MISUSE, SQLITE_OK, SQLITE_ROW, SQLITE_TRANSIENT,
    SQLITE_UTF8, sqlite3, sqlite3_bind_blob64, sqlite3_bind_double, sqlite3_bind_int,
    sqlite3_bind_int64, sqlite3_bind_null, sqlite3_bind_parameter_count,
    sqlite3_bind_parameter_name, sqlite3_bind_text64, sqlite3_changes, sqlite3_clear_bindings,
    sqlite3_column_blob, sqlite3_column_bytes, sqlite3_column_count, sqlite3_column_decltype,
    sqlite3_column_double, sqlite3_column_int64, sqlite3_column_name, sqlite3_column_type,
    sqlite3_db_handle, sqlite3_finalize, sqlite3_reset, sqlite3_step, sqlite3_stmt,
};

use crate::sqlite::SqliteError;
use crate::sqlite::type_info::SqliteDataType;

use super::unlock_notify;

#[derive(Debug)]
pub(crate) struct StatementHandle(NonNull<sqlite3_stmt>);

// access to SQLite3 statement handles are safe to send and share between threads
// as long as the `sqlite3_step` call is serialized.

unsafe impl Send for StatementHandle {}

impl StatementHandle {
    pub(super) fn new(ptr: NonNull<sqlite3_stmt>) -> Self {
        Self(ptr)
    }

    pub(super) unsafe fn db_handle(&self) -> *mut sqlite3 {
        // O(c) access to the connection handle for this statement handle
        // https://sqlite.org/c3ref/db_handle.html
        unsafe { sqlite3_db_handle(self.0.as_ptr()) }
    }

    pub(crate) fn last_error(&self) -> SqliteError {
        SqliteError::new(unsafe { self.db_handle() })
    }

    pub(crate) fn column_count(&self) -> usize {
        // https://sqlite.org/c3ref/column_count.html
        unsafe { sqlite3_column_count(self.0.as_ptr()) as usize }
    }

    pub(crate) fn changes(&self) -> u64 {
        // returns the number of changes of the *last* statement; not
        // necessarily this statement.
        // https://sqlite.org/c3ref/changes.html
        unsafe { sqlite3_changes(self.db_handle()) as u64 }
    }

    pub(crate) fn column_name(&self, index: usize) -> &str {
        // https://sqlite.org/c3ref/column_name.html
        unsafe {
            let name = sqlite3_column_name(self.0.as_ptr(), index as c_int);
            debug_assert!(!name.is_null());

            from_utf8_unchecked(CStr::from_ptr(name).to_bytes())
        }
    }

    pub(crate) fn column_type_info(&self, index: usize) -> SqliteDataType {
        SqliteDataType::from_code(self.column_type(index))
    }

    pub(crate) fn column_decltype(&self, index: usize) -> Option<SqliteDataType> {
        unsafe {
            let decl = sqlite3_column_decltype(self.0.as_ptr(), index as c_int);
            if decl.is_null() {
                // If the Nth column of the result set is an expression or subquery,
                // then a NULL pointer is returned.
                return None;
            }

            let decl = from_utf8_unchecked(CStr::from_ptr(decl).to_bytes());
            let ty: SqliteDataType = decl.parse().ok()?;

            Some(ty)
        }
    }

    // Number Of SQL Parameters

    pub(crate) fn bind_parameter_count(&self) -> usize {
        // https://www.sqlite.org/c3ref/bind_parameter_count.html
        unsafe { sqlite3_bind_parameter_count(self.0.as_ptr()) as usize }
    }

    // Name Of A Host Parameter
    // NOTE: The first host parameter has an index of 1, not 0.

    pub(crate) fn bind_parameter_name(&self, index: usize) -> Option<&str> {
        unsafe {
            // https://www.sqlite.org/c3ref/bind_parameter_name.html
            let name = sqlite3_bind_parameter_name(self.0.as_ptr(), index as c_int);
            if name.is_null() {
                return None;
            }

            Some(from_utf8_unchecked(CStr::from_ptr(name).to_bytes()))
        }
    }

    // Binding Values To Prepared Statements
    // https://www.sqlite.org/c3ref/bind_blob.html

    pub(crate) fn bind_blob(&self, index: usize, v: &[u8]) -> c_int {
        unsafe {
            sqlite3_bind_blob64(
                self.0.as_ptr(),
                index as c_int,
                v.as_ptr() as *const c_void,
                v.len() as u64,
                SQLITE_TRANSIENT(),
            )
        }
    }

    pub(crate) fn bind_text(&self, index: usize, v: &str) -> c_int {
        unsafe {
            sqlite3_bind_text64(
                self.0.as_ptr(),
                index as c_int,
                v.as_ptr() as *const c_char,
                v.len() as u64,
                SQLITE_TRANSIENT(),
                SQLITE_UTF8 as u8,
            )
        }
    }

    pub(crate) fn bind_int(&self, index: usize, v: i32) -> c_int {
        unsafe { sqlite3_bind_int(self.0.as_ptr(), index as c_int, v as c_int) }
    }

    pub(crate) fn bind_int64(&self, index: usize, v: i64) -> c_int {
        unsafe { sqlite3_bind_int64(self.0.as_ptr(), index as c_int, v) }
    }

    pub(crate) fn bind_double(&self, index: usize, v: f64) -> c_int {
        unsafe { sqlite3_bind_double(self.0.as_ptr(), index as c_int, v) }
    }

    pub(crate) fn bind_null(&self, index: usize) -> c_int {
        unsafe { sqlite3_bind_null(self.0.as_ptr(), index as c_int) }
    }

    // result values from the query
    // https://www.sqlite.org/c3ref/column_blob.html

    pub(crate) fn column_type(&self, index: usize) -> c_int {
        unsafe { sqlite3_column_type(self.0.as_ptr(), index as c_int) }
    }

    pub(crate) fn column_int64(&self, index: usize) -> i64 {
        unsafe { sqlite3_column_int64(self.0.as_ptr(), index as c_int) }
    }

    pub(crate) fn column_double(&self, index: usize) -> f64 {
        unsafe { sqlite3_column_double(self.0.as_ptr(), index as c_int) }
    }

    pub(crate) fn column_blob(&self, index: usize) -> *const c_void {
        unsafe { sqlite3_column_blob(self.0.as_ptr(), index as c_int) }
    }

    pub(crate) fn column_bytes(&self, index: usize) -> i32 {
        unsafe { sqlite3_column_bytes(self.0.as_ptr(), index as c_int) }
    }

    pub(crate) fn clear_bindings(&self) {
        unsafe { sqlite3_clear_bindings(self.0.as_ptr()) };
    }

    pub(crate) fn reset(&mut self) -> Result<(), SqliteError> {
        // SAFETY: we have exclusive access to the handle
        unsafe {
            if sqlite3_reset(self.0.as_ptr()) != SQLITE_OK {
                return Err(SqliteError::new(self.db_handle()));
            }
        }

        Ok(())
    }

    pub(crate) fn step(&mut self) -> Result<bool, crate::Error> {
        // SAFETY: we have exclusive access to the handle
        unsafe {
            loop {
                match sqlite3_step(self.0.as_ptr()) {
                    SQLITE_ROW => return Ok(true),
                    SQLITE_DONE => return Ok(false),
                    SQLITE_MISUSE => panic!("misuse!"),
                    SQLITE_LOCKED_SHAREDCACHE => {
                        // The shared cache is locked by another connection. Wait for unlock
                        // notification and try again.
                        unlock_notify::wait(self.db_handle(), Some(self.0.as_ptr()))?;
                        // Need to reset the handle after the unlock
                        // (https://www.sqlite.org/unlock_notify.html)
                        sqlite3_reset(self.0.as_ptr());
                    }
                    libsqlite3_sys::SQLITE_BUSY => {
                        // Another connection holds a lock that prevented the step from
                        // completing. Wait for an unlock notification and retry.
                        unlock_notify::wait(self.db_handle(), Some(self.0.as_ptr()))?;
                        sqlite3_reset(self.0.as_ptr());
                    }
                    _ => return Err(SqliteError::new(self.db_handle()).into()),
                }
            }
        }
    }
}

impl Drop for StatementHandle {
    fn drop(&mut self) {
        // SAFETY: we have exclusive access to the `StatementHandle` here
        unsafe {
            let db = self.db_handle();

            // Ensure the statement is reset before finalizing so that
            // sqlite3_finalize does not return SQLITE_BUSY.
            let reset_status = sqlite3_reset(self.0.as_ptr());
            if reset_status != SQLITE_OK {
                tracing::error!(
                    "sqlite3_reset before finalize failed: {}",
                    SqliteError::new(db)
                );
            }

            // https://sqlite.org/c3ref/finalize.html
            let status = sqlite3_finalize(self.0.as_ptr());
            if status == SQLITE_MISUSE {
                // Panic in case of detected misuse of SQLite API.
                //
                // sqlite3_finalize returns it at least in the
                // case of detected double free, i.e. calling
                // sqlite3_finalize on already finalized
                // statement.
                panic!("Detected sqlite3_finalize misuse.");
            } else if status != SQLITE_OK {
                tracing::error!("sqlite3_finalize failed: {}", SqliteError::new(db));
            }
        }
    }
}
