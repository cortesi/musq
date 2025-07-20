// Safe wrappers around libsqlite3_sys functions used within this crate.
// These wrappers centralize the `unsafe` blocks needed when calling into
// the SQLite C API so that the rest of the codebase can remain safe.

use std::ffi::c_void;
use std::os::raw::{c_char, c_int};
use std::ptr;

use libsqlite3_sys::{self as ffi_sys, sqlite3, sqlite3_stmt};

/// Wrapper around [`sqlite3_close`].
pub(crate) fn close(db: *mut sqlite3) -> c_int {
    unsafe { ffi_sys::sqlite3_close(db) }
}

/// Wrapper around [`sqlite3_exec`] with no callback.
pub(crate) fn exec(db: *mut sqlite3, sql: *const c_char) -> c_int {
    unsafe { ffi_sys::sqlite3_exec(db, sql, None, ptr::null_mut(), ptr::null_mut()) }
}

/// Wrapper around [`sqlite3_last_insert_rowid`].
pub(crate) fn last_insert_rowid(db: *mut sqlite3) -> i64 {
    unsafe { ffi_sys::sqlite3_last_insert_rowid(db) }
}

/// Wrapper around [`sqlite3_db_handle`].
pub(crate) fn db_handle(stmt: *mut sqlite3_stmt) -> *mut sqlite3 {
    unsafe { ffi_sys::sqlite3_db_handle(stmt) }
}

/// Wrapper around [`sqlite3_column_count`].
pub(crate) fn column_count(stmt: *mut sqlite3_stmt) -> c_int {
    unsafe { ffi_sys::sqlite3_column_count(stmt) }
}

/// Wrapper around [`sqlite3_changes`].
pub(crate) fn changes(db: *mut sqlite3) -> c_int {
    unsafe { ffi_sys::sqlite3_changes(db) }
}

/// Wrapper around [`sqlite3_column_name`]. Returns a pointer to a null terminated string.
pub(crate) fn column_name(stmt: *mut sqlite3_stmt, index: c_int) -> *const c_char {
    unsafe { ffi_sys::sqlite3_column_name(stmt, index) }
}

/// Wrapper around [`sqlite3_column_decltype`].
pub(crate) fn column_decltype(stmt: *mut sqlite3_stmt, index: c_int) -> *const c_char {
    unsafe { ffi_sys::sqlite3_column_decltype(stmt, index) }
}

/// Wrapper around [`sqlite3_bind_parameter_count`].
pub(crate) fn bind_parameter_count(stmt: *mut sqlite3_stmt) -> c_int {
    unsafe { ffi_sys::sqlite3_bind_parameter_count(stmt) }
}

/// Wrapper around [`sqlite3_bind_parameter_name`].
pub(crate) fn bind_parameter_name(stmt: *mut sqlite3_stmt, index: c_int) -> *const c_char {
    unsafe { ffi_sys::sqlite3_bind_parameter_name(stmt, index) }
}

/// Wrapper around [`sqlite3_bind_blob64`].
pub(crate) fn bind_blob64(
    stmt: *mut sqlite3_stmt,
    index: c_int,
    data: *const c_void,
    len: u64,
) -> c_int {
    unsafe { ffi_sys::sqlite3_bind_blob64(stmt, index, data, len, ffi_sys::SQLITE_TRANSIENT()) }
}

/// Wrapper around [`sqlite3_bind_text64`].
pub(crate) fn bind_text64(
    stmt: *mut sqlite3_stmt,
    index: c_int,
    data: *const c_char,
    len: u64,
) -> c_int {
    unsafe {
        ffi_sys::sqlite3_bind_text64(
            stmt,
            index,
            data,
            len,
            ffi_sys::SQLITE_TRANSIENT(),
            ffi_sys::SQLITE_UTF8 as u8,
        )
    }
}

/// Wrapper around [`sqlite3_bind_int`].
pub(crate) fn bind_int(stmt: *mut sqlite3_stmt, index: c_int, value: c_int) -> c_int {
    unsafe { ffi_sys::sqlite3_bind_int(stmt, index, value) }
}

/// Wrapper around [`sqlite3_bind_int64`].
pub(crate) fn bind_int64(stmt: *mut sqlite3_stmt, index: c_int, value: i64) -> c_int {
    unsafe { ffi_sys::sqlite3_bind_int64(stmt, index, value) }
}

/// Wrapper around [`sqlite3_bind_double`].
pub(crate) fn bind_double(stmt: *mut sqlite3_stmt, index: c_int, value: f64) -> c_int {
    unsafe { ffi_sys::sqlite3_bind_double(stmt, index, value) }
}

/// Wrapper around [`sqlite3_bind_null`].
pub(crate) fn bind_null(stmt: *mut sqlite3_stmt, index: c_int) -> c_int {
    unsafe { ffi_sys::sqlite3_bind_null(stmt, index) }
}

/// Wrapper around [`sqlite3_column_type`].
pub(crate) fn column_type(stmt: *mut sqlite3_stmt, index: c_int) -> c_int {
    unsafe { ffi_sys::sqlite3_column_type(stmt, index) }
}

/// Wrapper around [`sqlite3_column_int64`].
pub(crate) fn column_int64(stmt: *mut sqlite3_stmt, index: c_int) -> i64 {
    unsafe { ffi_sys::sqlite3_column_int64(stmt, index) }
}

/// Wrapper around [`sqlite3_column_double`].
pub(crate) fn column_double(stmt: *mut sqlite3_stmt, index: c_int) -> f64 {
    unsafe { ffi_sys::sqlite3_column_double(stmt, index) }
}

/// Wrapper around [`sqlite3_column_blob`].
pub(crate) fn column_blob(stmt: *mut sqlite3_stmt, index: c_int) -> *const c_void {
    unsafe { ffi_sys::sqlite3_column_blob(stmt, index) }
}

/// Wrapper around [`sqlite3_column_bytes`].
pub(crate) fn column_bytes(stmt: *mut sqlite3_stmt, index: c_int) -> c_int {
    unsafe { ffi_sys::sqlite3_column_bytes(stmt, index) }
}

/// Wrapper around [`sqlite3_clear_bindings`].
pub(crate) fn clear_bindings(stmt: *mut sqlite3_stmt) {
    unsafe { ffi_sys::sqlite3_clear_bindings(stmt) };
}

/// Wrapper around [`sqlite3_reset`].
pub(crate) fn reset(stmt: *mut sqlite3_stmt) -> c_int {
    unsafe { ffi_sys::sqlite3_reset(stmt) }
}

/// Wrapper around [`sqlite3_step`].
pub(crate) fn step(stmt: *mut sqlite3_stmt) -> c_int {
    unsafe { ffi_sys::sqlite3_step(stmt) }
}

/// Wrapper around [`sqlite3_finalize`].
pub(crate) fn finalize(stmt: *mut sqlite3_stmt) -> c_int {
    unsafe { ffi_sys::sqlite3_finalize(stmt) }
}
