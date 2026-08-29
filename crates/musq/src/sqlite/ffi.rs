// FFI wrappers around libsqlite3_sys. Raw-pointer entry points are `unsafe fn`.
// ConnectionHandle and StatementHandle are the safe boundary for those calls.

#[cfg(feature = "vec")]
use std::mem::transmute;
#[cfg(feature = "vec")]
use std::sync::OnceLock;
use std::{
    ffi::c_void,
    mem::size_of,
    os::raw::{c_char, c_int},
    ptr,
    result::Result as StdResult,
};

use libsqlite3_sys::{self as ffi_sys, sqlite3, sqlite3_backup, sqlite3_stmt};
#[cfg(feature = "vec")]
use sqlite_vec::sqlite3_vec_init;

use crate::sqlite::error::SqliteError;

// A compile-time assertion to ensure that `c_int` is 32 bits.
const _: () = {
    assert!(size_of::<c_int>() == 4);
};

/// Signature expected by SQLite for extension auto-registration.
#[cfg(feature = "vec")]
type ExtensionEntryPoint = unsafe extern "C" fn(
    db: *mut sqlite3,
    pz_err_msg: *mut *mut c_char,
    api: *const ffi_sys::sqlite3_api_routines,
) -> c_int;

/// Cached result of sqlite-vec auto-registration.
#[cfg(feature = "vec")]
static REGISTER_VEC_RESULT: OnceLock<StdResult<(), SqliteError>> = OnceLock::new();

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
pub(in crate::sqlite) unsafe fn open_v2(
    filename: *const c_char,
    handle: *mut *mut sqlite3,
    flags: i32,
    vfs: *const c_char,
) -> StdResult<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_open_v2(filename, handle, flags as c_int, vfs) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        // handle may be null on OOM
        let db = unsafe { *handle };
        if db.is_null() {
            Err(SqliteError::from_code(rc, "sqlite3_open_v2 failed"))
        } else {
            // capture the error before closing the handle
            let err = SqliteError::new(db);

            // SAFETY: db is valid when rc != SQLITE_OK and not null
            unsafe {
                ffi_sys::sqlite3_close(db);
                // prevent dangling pointer in the caller
                *handle = ptr::null_mut();
            }

            Err(err)
        }
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
pub(in crate::sqlite) unsafe fn busy_timeout(
    db: *mut sqlite3,
    ms: i32,
) -> StdResult<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_busy_timeout(db, ms as c_int) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// `sqlite3_db_config` operations that take `(int, int*)`.
#[derive(Clone, Copy)]
pub(in crate::sqlite) enum DbConfigIntOp {
    /// `SQLITE_DBCONFIG_FP_DIGITS`.
    FpDigits,
    /// `SQLITE_DBCONFIG_DQS_DDL`.
    DqsDdl,
    /// `SQLITE_DBCONFIG_DQS_DML`.
    DqsDml,
    /// `SQLITE_DBCONFIG_TRUSTED_SCHEMA`.
    TrustedSchema,
    /// `SQLITE_DBCONFIG_DEFENSIVE`.
    Defensive,
    /// `SQLITE_DBCONFIG_ENABLE_FKEY`.
    #[allow(dead_code)]
    EnableFkey,
    /// `SQLITE_DBCONFIG_ENABLE_TRIGGER`.
    #[allow(dead_code)]
    EnableTrigger,
    /// `SQLITE_DBCONFIG_ENABLE_VIEW`.
    #[allow(dead_code)]
    EnableView,
}

impl DbConfigIntOp {
    /// Return the SQLite `SQLITE_DBCONFIG_*` constant for this operation.
    fn as_raw(self) -> c_int {
        let code = match self {
            Self::FpDigits => ffi_sys::SQLITE_DBCONFIG_FP_DIGITS,
            Self::DqsDdl => ffi_sys::SQLITE_DBCONFIG_DQS_DDL,
            Self::DqsDml => ffi_sys::SQLITE_DBCONFIG_DQS_DML,
            Self::TrustedSchema => ffi_sys::SQLITE_DBCONFIG_TRUSTED_SCHEMA,
            Self::Defensive => ffi_sys::SQLITE_DBCONFIG_DEFENSIVE,
            Self::EnableFkey => ffi_sys::SQLITE_DBCONFIG_ENABLE_FKEY,
            Self::EnableTrigger => ffi_sys::SQLITE_DBCONFIG_ENABLE_TRIGGER,
            Self::EnableView => ffi_sys::SQLITE_DBCONFIG_ENABLE_VIEW,
        };
        code as c_int
    }
}

/// Wrapper around [`sqlite3_db_config`] for integer operations.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
///
/// See <https://www.sqlite.org/c3ref/c_dbconfig_defensive.html>.
#[inline]
#[must_use = "handle the Result"]
pub(in crate::sqlite) unsafe fn db_config_int(
    db: *mut sqlite3,
    op: DbConfigIntOp,
    value: i32,
) -> StdResult<i32, SqliteError> {
    let mut current = 0;
    let rc = unsafe {
        ffi_sys::sqlite3_db_config(db, op.as_raw(), value as c_int, &mut current as *mut c_int)
    };
    if rc == ffi_sys::SQLITE_OK {
        Ok(current)
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_limit`].
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
///
/// See <https://www.sqlite.org/c3ref/limit.html>.
#[inline]
pub(in crate::sqlite) unsafe fn limit(db: *mut sqlite3, id: i32, new_limit: i32) -> i32 {
    unsafe { ffi_sys::sqlite3_limit(db, id as c_int, new_limit as c_int) as i32 }
}

/// Return the SQLite runtime version number.
///
/// See <https://www.sqlite.org/c3ref/libversion.html>.
#[inline]
pub(in crate::sqlite) fn libversion_number() -> i32 {
    unsafe { ffi_sys::sqlite3_libversion_number() as i32 }
}

/// Wrapper around [`sqlite3_db_status64`].
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
///
/// See <https://www.sqlite.org/c3ref/db_status.html>.
#[inline]
#[must_use = "handle the Result"]
pub(in crate::sqlite) unsafe fn db_status64(
    db: *mut sqlite3,
    op: i32,
    reset_highwater: bool,
) -> StdResult<(i64, i64), SqliteError> {
    let mut current = 0_i64;
    let mut highwater = 0_i64;
    let rc = unsafe {
        ffi_sys::sqlite3_db_status64(
            db,
            op as c_int,
            &mut current,
            &mut highwater,
            reset_highwater as c_int,
        )
    };
    if rc == ffi_sys::SQLITE_OK {
        Ok((current, highwater))
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_wal_checkpoint_v2`].
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
/// - `schema` may be null or must point to a valid NUL terminated string.
///
/// See <https://www.sqlite.org/c3ref/wal_checkpoint_v2.html>.
#[inline]
#[must_use = "handle the Result"]
pub(in crate::sqlite) unsafe fn wal_checkpoint_v2(
    db: *mut sqlite3,
    schema: *const c_char,
    mode: i32,
) -> StdResult<(i32, i32), SqliteError> {
    let mut log_frames = 0_i32;
    let mut checkpointed_frames = 0_i32;
    let rc = unsafe {
        ffi_sys::sqlite3_wal_checkpoint_v2(
            db,
            schema,
            mode as c_int,
            &mut log_frames as *mut c_int,
            &mut checkpointed_frames as *mut c_int,
        )
    };
    if rc == ffi_sys::SQLITE_OK {
        Ok((log_frames, checkpointed_frames))
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
pub(in crate::sqlite) unsafe fn prepare_v3(
    db: *mut sqlite3,
    sql: *const c_char,
    n_byte: i32,
    flags: u32,
    stmt: *mut *mut sqlite3_stmt,
    tail: *mut *const c_char,
) -> StdResult<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_prepare_v3(db, sql, n_byte as c_int, flags, stmt, tail) };
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
pub(in crate::sqlite) unsafe fn extended_errcode(db: *mut sqlite3) -> i32 {
    unsafe { ffi_sys::sqlite3_extended_errcode(db) as i32 }
}

/// Wrapper around [`sqlite3_errmsg`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/errcode.html>
#[inline]
pub(in crate::sqlite) unsafe fn errmsg(db: *mut sqlite3) -> *const c_char {
    unsafe { ffi_sys::sqlite3_errmsg(db) }
}

/// Wrapper around [`sqlite3_error_offset`].
///
/// Returns the byte offset of the failing token, or `None` when SQLite does
/// not report an offset (`-1`).
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/errcode.html>
#[inline]
pub(in crate::sqlite) unsafe fn error_offset(db: *mut sqlite3) -> Option<usize> {
    let offset = unsafe { ffi_sys::sqlite3_error_offset(db) };
    if offset < 0 {
        None
    } else {
        Some(offset as usize)
    }
}

/// Wrapper around [`sqlite3_close`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/close.html>
#[inline]
#[must_use = "handle the Result"]
pub(in crate::sqlite) unsafe fn close(db: *mut sqlite3) -> StdResult<(), SqliteError> {
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
pub(in crate::sqlite) unsafe fn exec(
    db: *mut sqlite3,
    sql: *const c_char,
) -> StdResult<(), SqliteError> {
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
pub(in crate::sqlite) unsafe fn last_insert_rowid(db: *mut sqlite3) -> i64 {
    unsafe { ffi_sys::sqlite3_last_insert_rowid(db) }
}

/// Wrapper around [`sqlite3_get_autocommit`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/get_autocommit.html>
#[inline]
pub(in crate::sqlite) unsafe fn get_autocommit(db: *mut sqlite3) -> bool {
    unsafe { ffi_sys::sqlite3_get_autocommit(db) != 0 }
}

/// Wrapper around [`sqlite3_txn_state`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
/// - `schema` must be null or a valid UTF-8 C string.
///
/// See <https://www.sqlite.org/c3ref/txn_state.html>
#[inline]
pub(in crate::sqlite) unsafe fn txn_state(db: *mut sqlite3, schema: *const c_char) -> i32 {
    unsafe { ffi_sys::sqlite3_txn_state(db, schema) }
}

/// Wrapper around [`sqlite3_serialize`].
///
/// On success the pointer is owned by the caller and must be passed to
/// [`free`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
/// - `schema` must be a valid UTF-8 C string.
///
/// See <https://www.sqlite.org/c3ref/serialize.html>
#[inline]
#[must_use = "handle the Result"]
pub(in crate::sqlite) unsafe fn serialize(
    db: *mut sqlite3,
    schema: *const c_char,
) -> StdResult<(*mut u8, i64), SqliteError> {
    let mut size = 0_i64;
    let ptr = unsafe { ffi_sys::sqlite3_serialize(db, schema, &mut size, 0) };
    if ptr.is_null() {
        Err(SqliteError::new(db))
    } else {
        Ok((ptr, size))
    }
}

/// Wrapper around [`sqlite3_free`].
///
/// # Safety
/// - `ptr` must be null or a pointer allocated by SQLite.
///
/// See <https://www.sqlite.org/c3ref/free.html>
#[inline]
pub(in crate::sqlite) unsafe fn free(ptr: *mut c_void) {
    unsafe { ffi_sys::sqlite3_free(ptr) }
}

/// Wrapper around [`sqlite3_malloc64`].
///
/// # Safety
/// - The returned pointer, if non-null, must be passed to [`free`].
///
/// See <https://www.sqlite.org/c3ref/free.html>
#[inline]
#[must_use]
pub(in crate::sqlite) unsafe fn malloc64(size: u64) -> *mut c_void {
    unsafe { ffi_sys::sqlite3_malloc64(size) }
}

/// Wrapper around [`sqlite3_deserialize`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
/// - `schema` must be a valid UTF-8 C string.
/// - `data` must point to `buf_size` bytes allocated by SQLite when
///   `FREEONCLOSE` is set.
///
/// See <https://www.sqlite.org/c3ref/deserialize.html>
#[inline]
#[must_use = "handle the Result"]
pub(in crate::sqlite) unsafe fn deserialize(
    db: *mut sqlite3,
    schema: *const c_char,
    data: *mut u8,
    db_size: i64,
    buf_size: i64,
    flags: u32,
) -> StdResult<(), SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_deserialize(db, schema, data, db_size, buf_size, flags) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

/// Wrapper around [`sqlite3_db_filename`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
/// - `schema` must be a valid UTF-8 C string.
///
/// See <https://www.sqlite.org/c3ref/db_filename.html>
#[inline]
pub(in crate::sqlite) unsafe fn db_filename(
    db: *mut sqlite3,
    schema: *const c_char,
) -> *const c_char {
    unsafe { ffi_sys::sqlite3_db_filename(db, schema) }
}

/// Wrapper around [`sqlite3_backup_init`].
///
/// # Safety
/// - `dest` and `source` must be valid SQLite connection handles.
/// - `dest_name` and `source_name` must be valid UTF-8 C strings.
///
/// See <https://www.sqlite.org/c3ref/backup_finish.html>
#[inline]
pub(in crate::sqlite) unsafe fn backup_init(
    dest: *mut sqlite3,
    dest_name: *const c_char,
    source: *mut sqlite3,
    source_name: *const c_char,
) -> *mut sqlite3_backup {
    unsafe { ffi_sys::sqlite3_backup_init(dest, dest_name, source, source_name) }
}

/// Wrapper around [`sqlite3_backup_step`].
///
/// # Safety
/// - `backup` must be a valid backup handle from [`backup_init`].
///
/// See <https://www.sqlite.org/c3ref/backup_finish.html>
#[inline]
pub(in crate::sqlite) unsafe fn backup_step(backup: *mut sqlite3_backup, pages: i32) -> c_int {
    unsafe { ffi_sys::sqlite3_backup_step(backup, pages as c_int) }
}

/// Wrapper around [`sqlite3_backup_finish`].
///
/// # Safety
/// - `backup` must be a valid backup handle from [`backup_init`].
///
/// See <https://www.sqlite.org/c3ref/backup_finish.html>
#[inline]
pub(in crate::sqlite) unsafe fn backup_finish(backup: *mut sqlite3_backup) -> c_int {
    unsafe { ffi_sys::sqlite3_backup_finish(backup) }
}

/// Wrapper around [`sqlite3_backup_remaining`].
///
/// # Safety
/// - `backup` must be a valid backup handle from [`backup_init`].
///
/// See <https://www.sqlite.org/c3ref/backup_finish.html>
#[inline]
pub(in crate::sqlite) unsafe fn backup_remaining(backup: *mut sqlite3_backup) -> i32 {
    unsafe { ffi_sys::sqlite3_backup_remaining(backup) }
}

/// Wrapper around [`sqlite3_backup_pagecount`].
///
/// # Safety
/// - `backup` must be a valid backup handle from [`backup_init`].
///
/// See <https://www.sqlite.org/c3ref/backup_finish.html>
#[inline]
pub(in crate::sqlite) unsafe fn backup_pagecount(backup: *mut sqlite3_backup) -> i32 {
    unsafe { ffi_sys::sqlite3_backup_pagecount(backup) }
}

/// Wrapper around [`sqlite3_interrupt`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/interrupt.html>
#[inline]
pub(in crate::sqlite) unsafe fn interrupt(db: *mut sqlite3) {
    unsafe { ffi_sys::sqlite3_interrupt(db) }
}

/// Wrapper around [`sqlite3_progress_handler`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
/// - `x_progress` and `p_arg` must remain valid until the handler is cleared.
///
/// See <https://www.sqlite.org/c3ref/progress_handler.html>
#[inline]
pub(in crate::sqlite) unsafe fn progress_handler(
    db: *mut sqlite3,
    n: i32,
    x_progress: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    p_arg: *mut c_void,
) {
    unsafe { ffi_sys::sqlite3_progress_handler(db, n as c_int, x_progress, p_arg) }
}

/// Wrapper around [`sqlite3_db_handle`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/db_handle.html>
#[inline]
pub(in crate::sqlite) unsafe fn db_handle(stmt: *mut sqlite3_stmt) -> *mut sqlite3 {
    unsafe { ffi_sys::sqlite3_db_handle(stmt) }
}

/// Wrapper around [`sqlite3_column_count`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_count.html>
#[inline]
pub(in crate::sqlite) unsafe fn column_count(stmt: *mut sqlite3_stmt) -> i32 {
    unsafe { ffi_sys::sqlite3_column_count(stmt) as i32 }
}

/// Wrapper around [`sqlite3_stmt_readonly`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/stmt_readonly.html>
#[inline]
pub(in crate::sqlite) unsafe fn stmt_readonly(stmt: *mut sqlite3_stmt) -> bool {
    unsafe { ffi_sys::sqlite3_stmt_readonly(stmt) != 0 }
}

/// Wrapper around [`sqlite3_changes64`].
///
/// # Safety
/// - `db` must be a valid SQLite connection handle.
///
/// See <https://www.sqlite.org/c3ref/changes.html>
#[inline]
pub(in crate::sqlite) unsafe fn changes64(db: *mut sqlite3) -> i64 {
    unsafe { ffi_sys::sqlite3_changes64(db) }
}

/// Wrapper around [`sqlite3_auto_extension`].
///
/// Registers an extension entry point so SQLite loads it automatically for all
/// subsequently opened connections.
///
/// See <https://www.sqlite.org/c3ref/auto_extension.html>
#[cfg(feature = "vec")]
#[inline]
#[must_use = "handle the Result"]
pub(in crate::sqlite) fn auto_extension(
    entry_point: Option<ExtensionEntryPoint>,
) -> StdResult<(), i32> {
    let rc = unsafe { ffi_sys::sqlite3_auto_extension(entry_point) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(rc)
    }
}

/// Register sqlite-vec as an auto extension exactly once.
///
/// The first call attempts registration. Subsequent calls reuse that result.
#[cfg(feature = "vec")]
pub(in crate::sqlite) fn register_vec() -> crate::Result<()> {
    let result = REGISTER_VEC_RESULT.get_or_init(|| {
        // sqlite-vec exposes `sqlite3_vec_init` without a typed signature.
        // SQLite expects the canonical extension init function type here.
        let entry_point: ExtensionEntryPoint =
            unsafe { transmute::<unsafe extern "C" fn(), ExtensionEntryPoint>(sqlite3_vec_init) };

        match auto_extension(Some(entry_point)) {
            Ok(()) => Ok(()),
            Err(rc) => Err(SqliteError::from_code(
                rc,
                format!("sqlite3_auto_extension(sqlite3_vec_init) failed with rc={rc}"),
            )),
        }
    });

    result.clone().map_err(crate::Error::from)
}

/// Wrapper around [`sqlite3_column_name`]. Returns a pointer to a NUL terminated string.
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_name.html>
#[inline]
pub(in crate::sqlite) unsafe fn column_name(stmt: *mut sqlite3_stmt, index: i32) -> *const c_char {
    unsafe { ffi_sys::sqlite3_column_name(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_decltype`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_decltype.html>
#[inline]
pub(in crate::sqlite) unsafe fn column_decltype(
    stmt: *mut sqlite3_stmt,
    index: i32,
) -> *const c_char {
    unsafe { ffi_sys::sqlite3_column_decltype(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_bind_parameter_count`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/bind_parameter_count.html>
#[inline]
pub(in crate::sqlite) unsafe fn bind_parameter_count(stmt: *mut sqlite3_stmt) -> i32 {
    unsafe { ffi_sys::sqlite3_bind_parameter_count(stmt) as i32 }
}

/// Wrapper around [`sqlite3_bind_parameter_name`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/bind_parameter_name.html>
#[inline]
pub(in crate::sqlite) unsafe fn bind_parameter_name(
    stmt: *mut sqlite3_stmt,
    index: i32,
) -> *const c_char {
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
pub(in crate::sqlite) unsafe fn bind_blob64(
    stmt: *mut sqlite3_stmt,
    index: i32,
    data: *const c_void,
    len: u64,
) -> StdResult<(), SqliteError> {
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
pub(in crate::sqlite) unsafe fn bind_text64(
    stmt: *mut sqlite3_stmt,
    index: i32,
    data: *const c_char,
    len: u64,
) -> StdResult<(), SqliteError> {
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
pub(in crate::sqlite) unsafe fn bind_int64(
    stmt: *mut sqlite3_stmt,
    index: i32,
    value: i64,
) -> StdResult<(), SqliteError> {
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
pub(in crate::sqlite) unsafe fn bind_double(
    stmt: *mut sqlite3_stmt,
    index: i32,
    value: f64,
) -> StdResult<(), SqliteError> {
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
pub(in crate::sqlite) unsafe fn bind_null(
    stmt: *mut sqlite3_stmt,
    index: i32,
) -> StdResult<(), SqliteError> {
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
pub(in crate::sqlite) unsafe fn column_type(stmt: *mut sqlite3_stmt, index: i32) -> i32 {
    unsafe { ffi_sys::sqlite3_column_type(stmt, index as c_int) as i32 }
}

/// Wrapper around [`sqlite3_column_int64`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(in crate::sqlite) unsafe fn column_int64(stmt: *mut sqlite3_stmt, index: i32) -> i64 {
    unsafe { ffi_sys::sqlite3_column_int64(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_double`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(in crate::sqlite) unsafe fn column_double(stmt: *mut sqlite3_stmt, index: i32) -> f64 {
    unsafe { ffi_sys::sqlite3_column_double(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_text`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(in crate::sqlite) unsafe fn column_text(stmt: *mut sqlite3_stmt, index: i32) -> *const u8 {
    unsafe { ffi_sys::sqlite3_column_text(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_blob`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(in crate::sqlite) unsafe fn column_blob(stmt: *mut sqlite3_stmt, index: i32) -> *const c_void {
    unsafe { ffi_sys::sqlite3_column_blob(stmt, index as c_int) }
}

/// Wrapper around [`sqlite3_column_bytes`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/column_blob.html>
#[inline]
pub(in crate::sqlite) unsafe fn column_bytes(stmt: *mut sqlite3_stmt, index: i32) -> i32 {
    unsafe { ffi_sys::sqlite3_column_bytes(stmt, index as c_int) as i32 }
}

/// Wrapper around [`sqlite3_clear_bindings`].
///
/// # Safety
/// - `stmt` must be a valid prepared statement pointer.
///
/// See <https://www.sqlite.org/c3ref/clear_bindings.html>
#[inline]
pub(in crate::sqlite) unsafe fn clear_bindings(stmt: *mut sqlite3_stmt) {
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
pub(in crate::sqlite) unsafe fn reset(stmt: *mut sqlite3_stmt) -> StdResult<(), SqliteError> {
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
pub(in crate::sqlite) unsafe fn step(stmt: *mut sqlite3_stmt) -> StdResult<i32, SqliteError> {
    let rc = unsafe { ffi_sys::sqlite3_step(stmt) };
    if rc == ffi_sys::SQLITE_ROW || rc == ffi_sys::SQLITE_DONE {
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
pub(in crate::sqlite) unsafe fn finalize(stmt: *mut sqlite3_stmt) -> StdResult<(), SqliteError> {
    // sqlite3_finalize frees the statement even when it returns an error, so
    // the database pointer must be captured first.
    let db = unsafe { ffi_sys::sqlite3_db_handle(stmt) };
    let rc = unsafe { ffi_sys::sqlite3_finalize(stmt) };
    if rc == ffi_sys::SQLITE_OK {
        Ok(())
    } else {
        Err(SqliteError::new(db))
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::CString, ptr};

    use super::*;
    use crate::sqlite::error::PrimaryErrCode;

    #[test]
    fn basic_open_prepare_step_reset_finalize() {
        // SAFETY: this test owns the connection and statement pointers it opens.
        unsafe {
            let filename = CString::new(":memory:").unwrap();
            let mut handle = ptr::null_mut();
            open_v2(
                filename.as_ptr(),
                &mut handle,
                libsqlite3_sys::SQLITE_OPEN_READWRITE
                    | libsqlite3_sys::SQLITE_OPEN_CREATE
                    | libsqlite3_sys::SQLITE_OPEN_MEMORY
                    | libsqlite3_sys::SQLITE_OPEN_EXRESCODE,
                ptr::null(),
            )
            .unwrap();
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
            assert_eq!(changes64(handle), 1);
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
            let count = libsqlite3_sys::sqlite3_column_int(stmt, 0);
            finalize(stmt).unwrap();
            assert_eq!(count, 1);

            close(handle).unwrap();
        }
    }

    #[test]
    fn finalize_after_failed_step_reads_error_from_connection() {
        // SAFETY: this test owns the connection and statement pointers it opens.
        unsafe {
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

            let create_sql = CString::new("CREATE TABLE t (id INTEGER PRIMARY KEY);").unwrap();
            exec(handle, create_sql.as_ptr()).unwrap();
            let insert_sql = CString::new("INSERT INTO t VALUES (1);").unwrap();
            exec(handle, insert_sql.as_ptr()).unwrap();

            let dup_sql = CString::new("INSERT INTO t VALUES (1);").unwrap();
            let mut stmt = ptr::null_mut();
            prepare_v3(handle, dup_sql.as_ptr(), -1, 0, &mut stmt, ptr::null_mut()).unwrap();
            assert!(step(stmt).is_err());

            let err = finalize(stmt).expect_err("finalize should report the last step error");
            assert_eq!(err.primary, PrimaryErrCode::Constraint);
            assert!(
                err.message.to_ascii_lowercase().contains("unique")
                    || err.message.to_ascii_lowercase().contains("constraint"),
                "unexpected message: {}",
                err.message
            );

            close(handle).unwrap();
        }
    }
}
