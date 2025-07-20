// Safe wrappers around libsqlite3_sys functions used within this crate.
// These wrappers centralize the `unsafe` blocks needed when calling into
// the SQLite C API so that the rest of the codebase can remain safe.

use std::ffi::c_void;
use std::os::raw::{c_char, c_int};
use std::ptr;

use crate::sqlite::error::{ExtendedErrCode, PrimaryErrCode, SqliteError};
use libsqlite3_sys::{self as ffi_sys, sqlite3, sqlite3_stmt};

// A compile-time assertion to ensure that `c_int` is 32 bits.
const _: () = {
    assert!(std::mem::size_of::<c_int>() == 4);
};

/// Wrapper around [`sqlite3_open_v2`].
///
/// # Safety
/// - `filename` must point to a valid NUL terminated string.
/// - `handle` must be a valid pointer to receive the database handle and must
///   not be accessed concurrently.
/// - `vfs` may be null or must point to a valid NUL terminated string.
///
/// See <https://www.sqlite.org/c3ref/open.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn open_v2(
    filename: *const c_char,
    handle: *mut *mut sqlite3,
    flags: i32,
    vfs: *const c_char,
) -> std::result::Result<(), SqliteError> {
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
            // capture the error before closing the handle
            let err = SqliteError::new(db);

            // SAFETY: db is valid when rc != SQLITE_OK and not null
            unsafe {
                ffi_sys::sqlite3_close(db);
                // prevent dangling pointer in the caller
                *handle = std::ptr::null_mut();
            }

            Err(err)
        }
    }
}

/// Wrapper around [`sqlite3_extended_result_codes`].
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
///
/// See <https://www.sqlite.org/c3ref/errcode.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn extended_result_codes(
    db: *mut sqlite3,
    onoff: i32,
) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_extended_result_codes(db, onoff as c_int) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_busy_timeout`].
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
///
/// See <https://www.sqlite.org/c3ref/busy_timeout.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn busy_timeout(db: *mut sqlite3, ms: i32) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_busy_timeout(db, ms as c_int) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_prepare_v3`].
///
/// # Safety
/// - `db` must be a valid SQLite database handle.
/// - `sql` must point to a valid SQL statement with at least `n_byte` bytes.
/// - `stmt` and `tail` must be valid pointers to receive output.
///
/// See <https://www.sqlite.org/c3ref/prepare.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn prepare_v3(
    db: *mut sqlite3,
    sql: *const c_char,
    n_byte: i32,
    flags: u32,
    stmt: *mut *mut sqlite3_stmt,
    tail: *mut *const c_char,
) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_prepare_v3(db, sql, n_byte as c_int, flags, stmt, tail) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_unlock_notify`].
///
/// # Safety
/// - `db` must be a valid SQLite handle.
/// - `callback` must remain valid until invocation.
///
/// See <https://www.sqlite.org/c3ref/unlock_notify.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn unlock_notify(
    db: *mut sqlite3,
    callback: Option<unsafe extern "C" fn(*mut *mut c_void, c_int)>,
    arg: *mut c_void,
) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_unlock_notify(db, callback, arg) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_extended_errcode`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/errcode.html>
#[inline]
pub(crate) fn extended_errcode(db: *mut sqlite3) -> i32 {
    unsafe { ffi_sys::sqlite3_extended_errcode(db) as i32 }
}

/// Wrapper around [`sqlite3_errmsg`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/errcode.html>
#[inline]
pub(crate) fn errmsg(db: *mut sqlite3) -> *const c_char {
    unsafe { ffi_sys::sqlite3_errmsg(db) }
}

/// Wrapper around [`sqlite3_close`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/close.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn close(db: *mut sqlite3) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_close(db) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_exec`] with no callback.
///
/// # Safety
/// - `db` must be a valid SQLite connection.
/// - `sql` must point to a valid NUL terminated SQL statement.
///
/// See <https://www.sqlite.org/c3ref/exec.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn exec(db: *mut sqlite3, sql: *const c_char) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_exec(db, sql, None, ptr::null_mut(), ptr::null_mut()) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_last_insert_rowid`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/last_insert_rowid.html>
#[inline]
pub(crate) fn last_insert_rowid(db: *mut sqlite3) -> i64 {
    unsafe { ffi_sys::sqlite3_last_insert_rowid(db) }
}

/// Wrapper around [`sqlite3_db_handle`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/db_handle.html>
#[inline]
pub(crate) fn db_handle(stmt: *mut sqlite3_stmt) -> *mut sqlite3 {
    unsafe { ffi_sys::sqlite3_db_handle(stmt) }
}

/// Wrapper around [`sqlite3_column_count`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_count.html>
#[inline]
pub(crate) fn column_count(stmt: *mut sqlite3_stmt) -> i32 {
    unsafe { ffi_sys::sqlite3_column_count(stmt) as i32 }
}

/// Wrapper around [`sqlite3_changes`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/changes.html>
#[inline]
pub(crate) fn changes(db: *mut sqlite3) -> i32 {
    unsafe { ffi_sys::sqlite3_changes(db) as i32 }
}

/// Wrapper around [`sqlite3_column_name`]. Returns a pointer to a NUL terminated string.
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_name.html>
#[inline]
pub(crate) fn column_name(stmt: *mut sqlite3_stmt, index: i32) -> *const c_char {
    unsafe { ffi_sys::sqlite3_column_name(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_decltype`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_decltype.html>
#[inline]
pub(crate) fn column_decltype(stmt: *mut sqlite3_stmt, index: i32) -> *const c_char {
    unsafe { ffi_sys::sqlite3_column_decltype(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_bind_parameter_count`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/bind_parameter_count.html>
#[inline]
pub(crate) fn bind_parameter_count(stmt: *mut sqlite3_stmt) -> i32 {
    unsafe { ffi_sys::sqlite3_bind_parameter_count(stmt) as i32 }
}

/// Wrapper around [`sqlite3_bind_parameter_name`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/bind_parameter_name.html>
#[inline]
pub(crate) fn bind_parameter_name(stmt: *mut sqlite3_stmt, index: i32) -> *const c_char {
    unsafe { ffi_sys::sqlite3_bind_parameter_name(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_bind_blob64`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
/// - `data` must point to `len` valid bytes.
///
/// See <https://www.sqlite.org/c3ref/bind_blob.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn bind_blob64(
    stmt: *mut sqlite3_stmt,
    index: i32,
    data: *const c_void,
    len: u64,
) -> std::result::Result<(), SqliteError> {
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
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
/// - `data` must point to `len` bytes representing UTF-8 text.
///
/// See <https://www.sqlite.org/c3ref/bind_blob.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn bind_text64(
    stmt: *mut sqlite3_stmt,
    index: i32,
    data: *const c_char,
    len: u64,
) -> std::result::Result<(), SqliteError> {
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

/// Wrapper around [`sqlite3_bind_int64`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/bind_blob.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn bind_int64(
    stmt: *mut sqlite3_stmt,
    index: i32,
    value: i64,
) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_bind_int64(stmt, index as c_int, value) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_bind_double`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/bind_blob.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn bind_double(
    stmt: *mut sqlite3_stmt,
    index: i32,
    value: f64,
) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_bind_double(stmt, index as c_int, value) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_bind_null`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/bind_blob.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn bind_null(
    stmt: *mut sqlite3_stmt,
    index: i32,
) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_bind_null(stmt, index as c_int) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_column_type`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(crate) fn column_type(stmt: *mut sqlite3_stmt, index: i32) -> i32 {
    unsafe { ffi_sys::sqlite3_column_type(stmt, index as c_int) as i32 }
}

/// Wrapper around [`sqlite3_column_int64`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(crate) fn column_int64(stmt: *mut sqlite3_stmt, index: i32) -> i64 {
    unsafe { ffi_sys::sqlite3_column_int64(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_double`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(crate) fn column_double(stmt: *mut sqlite3_stmt, index: i32) -> f64 {
    unsafe { ffi_sys::sqlite3_column_double(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_blob`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(crate) fn column_blob(stmt: *mut sqlite3_stmt, index: i32) -> *const c_void {
    unsafe { ffi_sys::sqlite3_column_blob(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_bytes`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(crate) fn column_bytes(stmt: *mut sqlite3_stmt, index: i32) -> i32 {
    unsafe { ffi_sys::sqlite3_column_bytes(stmt, index as c_int) as i32 }
}

/// Wrapper around [`sqlite3_clear_bindings`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/clear_bindings.html>
#[inline]
pub(crate) fn clear_bindings(stmt: *mut sqlite3_stmt) {
    unsafe { ffi_sys::sqlite3_clear_bindings(stmt) };
}

/// Wrapper around [`sqlite3_reset`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/reset.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn reset(stmt: *mut sqlite3_stmt) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_reset(stmt) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_step`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/step.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn step(stmt: *mut sqlite3_stmt) -> std::result::Result<i32, SqliteError> {
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
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/finalize.html>
#[inline]
#[must_use = "handle the Result"]
pub(crate) fn finalize(stmt: *mut sqlite3_stmt) -> std::result::Result<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_finalize(stmt) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
        Err(SqliteError::new(db))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn basic_open_prepare_step_reset_finalize() {
        let filename = CString::new(":memory:").unwrap();
        let mut handle = ptr::null_mut();
        open_v2(
            filename.as_ptr(),
            &mut handle,
            libsqlite3_sys::SQLITE_OPEN_READWRITE
                | libsqlite3_sys::SQLITE_OPEN_CREATE
                | libsqlite3_sys::SQLITE_OPEN_MEMORY,
            ptr::null(),
        )
        .unwrap();
        extended_result_codes(handle, 1).unwrap();
        busy_timeout(handle, 1000).unwrap();

        let create_sql = CString::new("CREATE TABLE t (val TEXT);").unwrap();
        exec(handle, create_sql.as_ptr()).unwrap();

        let insert_sql = CString::new("INSERT INTO t (val) VALUES ('foo');").unwrap();
        let mut stmt = ptr::null_mut();
        prepare_v3(
            handle,
            insert_sql.as_ptr(),
            -1,
            0,
            &mut stmt,
            ptr::null_mut(),
        )
        .unwrap();
        assert_eq!(step(stmt).unwrap(), libsqlite3_sys::SQLITE_DONE);
        reset(stmt).unwrap();
        finalize(stmt).unwrap();

        let count_sql = CString::new("SELECT COUNT(*) FROM t;").unwrap();
        let mut stmt = ptr::null_mut();
        prepare_v3(
            handle,
            count_sql.as_ptr(),
            -1,
            0,
            &mut stmt,
            ptr::null_mut(),
        )
        .unwrap();
        assert_eq!(step(stmt).unwrap(), libsqlite3_sys::SQLITE_ROW);
        let count = unsafe { libsqlite3_sys::sqlite3_column_int(stmt, 0) };
        finalize(stmt).unwrap();
        assert_eq!(count, 1);

        close(handle).unwrap();
    }
}
