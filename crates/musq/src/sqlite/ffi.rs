// Safe wrappers around libsqlite3_sys functions used within this crate.
// These wrappers centralize the `unsafe` blocks needed when calling into
// the SQLite C API so that the rest of the codebase can remain safe.

use std::ffi::c_void;
use std::os::raw::{c_char, c_int};
use std::ptr;

use crate::sqlite::error::{ExtendedErrCode, PrimaryErrCode, SqliteError};
use libsqlite3_sys::{self as ffi_sys, sqlite3, sqlite3_stmt};

#[allow(dead_code)]
const fn assert_c_int_is_32bit() {
    assert!(std::mem::size_of::<c_int>() == 4);
}

// A compile-time assertion to ensure that `c_int` is 32 bits.
const _ASSERT_C_INT_32BIT: () = assert_c_int_is_32bit();

/// Wrapper around [`sqlite3_open_v2`].
pub(crate) fn open_v2(
    filename: *const c_char,
    handle: *mut *mut sqlite3,
    flags: i32,
    vfs: *const c_char,
) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_open_v2(filename, handle, flags as c_int, vfs) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        // handle may be null on OOM
        let db = unsafe { *handle };
        if db.is_null() {
            Err(SqliteError {
                primary: PrimaryErrCode::Unknown(rc as u32),
                extended: ExtendedErrCode::Unknown(rc as u32),
                message: "sqlite3_open_v2 failed".into(),
            })
        } else {
            // SAFETY: db is valid when rc != SQLITE_OK and not null
            unsafe {
                ffi_sys::sqlite3_close(db);
            }

            Err(SqliteError::new(db))
        }
    }
}

/// Wrapper around [`sqlite3_extended_result_codes`].
pub(crate) fn extended_result_codes(db: *mut sqlite3, onoff: i32) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_extended_result_codes(db, onoff as c_int) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_busy_timeout`].
pub(crate) fn busy_timeout(db: *mut sqlite3, ms: i32) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_busy_timeout(db, ms as c_int) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_prepare_v3`].
pub(crate) fn prepare_v3(
    db: *mut sqlite3,
    sql: *const c_char,
    n_byte: i32,
    flags: u32,
    stmt: *mut *mut sqlite3_stmt,
    tail: *mut *const c_char,
) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_prepare_v3(db, sql, n_byte as c_int, flags, stmt, tail) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_progress_handler`].
pub(crate) fn progress_handler(
    db: *mut sqlite3,
    num_ops: i32,
    callback: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    arg: *mut c_void,
) {
    unsafe {
        ffi_sys::sqlite3_progress_handler(db, num_ops as c_int, callback, arg);
    }
}

/// Wrapper around [`sqlite3_unlock_notify`].
pub(crate) fn unlock_notify(
    db: *mut sqlite3,
    callback: Option<unsafe extern "C" fn(*mut *mut c_void, c_int)>,
    arg: *mut c_void,
) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_unlock_notify(db, callback, arg) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_extended_errcode`].
pub(crate) fn extended_errcode(db: *mut sqlite3) -> i32 {
    unsafe { ffi_sys::sqlite3_extended_errcode(db) as i32 }
}

/// Wrapper around [`sqlite3_errmsg`].
pub(crate) fn errmsg(db: *mut sqlite3) -> *const c_char {
    unsafe { ffi_sys::sqlite3_errmsg(db) }
}

/// Wrapper around [`sqlite3_close`].
pub(crate) fn close(db: *mut sqlite3) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_close(db) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_exec`] with no callback.
pub(crate) fn exec(db: *mut sqlite3, sql: *const c_char) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_exec(db, sql, None, ptr::null_mut(), ptr::null_mut()) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
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
pub(crate) fn column_count(stmt: *mut sqlite3_stmt) -> i32 {
    unsafe { ffi_sys::sqlite3_column_count(stmt) as i32 }
}

/// Wrapper around [`sqlite3_changes`].
pub(crate) fn changes(db: *mut sqlite3) -> i32 {
    unsafe { ffi_sys::sqlite3_changes(db) as i32 }
}

/// Wrapper around [`sqlite3_column_name`]. Returns a pointer to a null terminated string.
pub(crate) fn column_name(stmt: *mut sqlite3_stmt, index: i32) -> *const c_char {
    unsafe { ffi_sys::sqlite3_column_name(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_decltype`].
pub(crate) fn column_decltype(stmt: *mut sqlite3_stmt, index: i32) -> *const c_char {
    unsafe { ffi_sys::sqlite3_column_decltype(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_bind_parameter_count`].
pub(crate) fn bind_parameter_count(stmt: *mut sqlite3_stmt) -> i32 {
    unsafe { ffi_sys::sqlite3_bind_parameter_count(stmt) as i32 }
}

/// Wrapper around [`sqlite3_bind_parameter_name`].
pub(crate) fn bind_parameter_name(stmt: *mut sqlite3_stmt, index: i32) -> *const c_char {
    unsafe { ffi_sys::sqlite3_bind_parameter_name(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_bind_blob64`].
pub(crate) fn bind_blob64(
    stmt: *mut sqlite3_stmt,
    index: i32,
    data: *const c_void,
    len: u64,
) -> Result<(), SqliteError> {
    let rc = unsafe {
        ffi_sys::sqlite3_bind_blob64(stmt, index, data, len, ffi_sys::SQLITE_TRANSIENT())
    };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_bind_text64`].
pub(crate) fn bind_text64(
    stmt: *mut sqlite3_stmt,
    index: i32,
    data: *const c_char,
    len: u64,
) -> Result<(), SqliteError> {
    unsafe {
        let rc = ffi_sys::sqlite3_bind_text64(
            stmt,
            index as c_int,
            data,
            len,
            ffi_sys::SQLITE_TRANSIENT(),
            ffi_sys::SQLITE_UTF8 as u8,
        );
        if rc == ffi_sys::SQLITE_OK {
            Ok(())
        } else {
            let db = ffi_sys::sqlite3_db_handle(stmt);
            Err(SqliteError::new(db))
        }
    }
}

/// Wrapper around [`sqlite3_bind_int`].
pub(crate) fn bind_int(stmt: *mut sqlite3_stmt, index: i32, value: i32) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_bind_int(stmt, index as c_int, value as c_int) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_bind_int64`].
pub(crate) fn bind_int64(
    stmt: *mut sqlite3_stmt,
    index: i32,
    value: i64,
) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_bind_int64(stmt, index as c_int, value) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_bind_double`].
pub(crate) fn bind_double(
    stmt: *mut sqlite3_stmt,
    index: i32,
    value: f64,
) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_bind_double(stmt, index as c_int, value) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_bind_null`].
pub(crate) fn bind_null(stmt: *mut sqlite3_stmt, index: i32) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_bind_null(stmt, index as c_int) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_column_type`].
pub(crate) fn column_type(stmt: *mut sqlite3_stmt, index: i32) -> i32 {
    unsafe { ffi_sys::sqlite3_column_type(stmt, index as c_int) as i32 }
}

/// Wrapper around [`sqlite3_column_int64`].
pub(crate) fn column_int64(stmt: *mut sqlite3_stmt, index: i32) -> i64 {
    unsafe { ffi_sys::sqlite3_column_int64(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_double`].
pub(crate) fn column_double(stmt: *mut sqlite3_stmt, index: i32) -> f64 {
    unsafe { ffi_sys::sqlite3_column_double(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_blob`].
pub(crate) fn column_blob(stmt: *mut sqlite3_stmt, index: i32) -> *const c_void {
    unsafe { ffi_sys::sqlite3_column_blob(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_bytes`].
pub(crate) fn column_bytes(stmt: *mut sqlite3_stmt, index: i32) -> i32 {
    unsafe { ffi_sys::sqlite3_column_bytes(stmt, index as c_int) as i32 }
}

/// Wrapper around [`sqlite3_clear_bindings`].
pub(crate) fn clear_bindings(stmt: *mut sqlite3_stmt) {
    unsafe { ffi_sys::sqlite3_clear_bindings(stmt) };
}

/// Wrapper around [`sqlite3_reset`].
pub(crate) fn reset(stmt: *mut sqlite3_stmt) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_reset(stmt) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_step`].
pub(crate) fn step(stmt: *mut sqlite3_stmt) -> Result<i32, SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_step(stmt) };
    if rc == ffi_sys::SQLITE_ROW
        || rc == ffi_sys::SQLITE_DONE
        || rc == ffi_sys::SQLITE_LOCKED_SHAREDCACHE
        || rc == ffi_sys::SQLITE_LOCKED
        || rc == ffi_sys::SQLITE_BUSY
        || rc == ffi_sys::SQLITE_MISUSE
        || rc == ffi_sys::SQLITE_OK
    {
        Ok(rc as i32)
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_finalize`].
pub(crate) fn finalize(stmt: *mut sqlite3_stmt) -> Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_finalize(stmt) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}
