//! Per-connection SQLite authorizer, update, commit, and rollback hooks.

use std::{
    borrow::Cow,
    ffi::{CStr, c_void},
    os::raw::{c_char, c_int},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use libsqlite3_sys::{
    self as ffi_sys, SQLITE_ALTER_TABLE, SQLITE_ANALYZE, SQLITE_ATTACH, SQLITE_COPY,
    SQLITE_CREATE_INDEX, SQLITE_CREATE_TABLE, SQLITE_CREATE_TEMP_INDEX, SQLITE_CREATE_TEMP_TABLE,
    SQLITE_CREATE_TEMP_TRIGGER, SQLITE_CREATE_TEMP_VIEW, SQLITE_CREATE_TRIGGER, SQLITE_CREATE_VIEW,
    SQLITE_CREATE_VTABLE, SQLITE_DELETE, SQLITE_DENY, SQLITE_DETACH, SQLITE_DROP_INDEX,
    SQLITE_DROP_TABLE, SQLITE_DROP_TEMP_INDEX, SQLITE_DROP_TEMP_TABLE, SQLITE_DROP_TEMP_TRIGGER,
    SQLITE_DROP_TEMP_VIEW, SQLITE_DROP_TRIGGER, SQLITE_DROP_VIEW, SQLITE_DROP_VTABLE,
    SQLITE_FUNCTION, SQLITE_IGNORE, SQLITE_INSERT, SQLITE_OK, SQLITE_PRAGMA, SQLITE_READ,
    SQLITE_RECURSIVE, SQLITE_REINDEX, SQLITE_SAVEPOINT, SQLITE_SELECT, SQLITE_TRANSACTION,
    SQLITE_UPDATE, sqlite3,
};

use crate::sqlite::{error::SqliteError, ffi};

/// Decision returned by a SQLite authorizer callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthResult {
    /// Allow the operation (`SQLITE_OK`).
    Ok,
    /// Deny the operation and abort statement preparation (`SQLITE_DENY`).
    Deny,
    /// Allow statement preparation, treating disallowed column reads as `NULL`
    /// or ignoring the action where SQLite permits it (`SQLITE_IGNORE`).
    Ignore,
}

impl AuthResult {
    /// Convert to the raw SQLite authorizer return code.
    fn as_raw(self) -> c_int {
        match self {
            Self::Ok => SQLITE_OK,
            Self::Deny => SQLITE_DENY,
            Self::Ignore => SQLITE_IGNORE,
        }
    }
}

/// Action code reported to a SQLite authorizer callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActionCode {
    /// Deprecated `COPY` action (`SQLITE_COPY`).
    Copy,
    /// `CREATE INDEX`.
    CreateIndex,
    /// `CREATE TABLE`.
    CreateTable,
    /// `CREATE TEMP INDEX`.
    CreateTempIndex,
    /// `CREATE TEMP TABLE`.
    CreateTempTable,
    /// `CREATE TEMP TRIGGER`.
    CreateTempTrigger,
    /// `CREATE TEMP VIEW`.
    CreateTempView,
    /// `CREATE TRIGGER`.
    CreateTrigger,
    /// `CREATE VIEW`.
    CreateView,
    /// `DELETE`.
    Delete,
    /// `DROP INDEX`.
    DropIndex,
    /// `DROP TABLE`.
    DropTable,
    /// `DROP TEMP INDEX`.
    DropTempIndex,
    /// `DROP TEMP TABLE`.
    DropTempTable,
    /// `DROP TEMP TRIGGER`.
    DropTempTrigger,
    /// `DROP TEMP VIEW`.
    DropTempView,
    /// `DROP TRIGGER`.
    DropTrigger,
    /// `DROP VIEW`.
    DropView,
    /// `INSERT`.
    Insert,
    /// `PRAGMA`.
    Pragma,
    /// Read a table column.
    Read,
    /// `SELECT`.
    Select,
    /// `BEGIN`, `COMMIT`, or `ROLLBACK`.
    Transaction,
    /// `UPDATE`.
    Update,
    /// `ATTACH`.
    Attach,
    /// `DETACH`.
    Detach,
    /// `ALTER TABLE`.
    AlterTable,
    /// `REINDEX`.
    Reindex,
    /// `ANALYZE`.
    Analyze,
    /// `CREATE VIRTUAL TABLE`.
    CreateVtable,
    /// `DROP VIRTUAL TABLE`.
    DropVtable,
    /// SQL function invocation (`arg2` is the function name).
    Function,
    /// `SAVEPOINT`, `RELEASE`, or `ROLLBACK TO`.
    Savepoint,
    /// Recursive CTE query.
    Recursive,
    /// Unrecognized SQLite action code.
    Unknown(i32),
}

impl ActionCode {
    /// Map a raw SQLite authorizer action code to [`ActionCode`].
    fn from_raw(code: c_int) -> Self {
        match code {
            SQLITE_COPY => Self::Copy,
            SQLITE_CREATE_INDEX => Self::CreateIndex,
            SQLITE_CREATE_TABLE => Self::CreateTable,
            SQLITE_CREATE_TEMP_INDEX => Self::CreateTempIndex,
            SQLITE_CREATE_TEMP_TABLE => Self::CreateTempTable,
            SQLITE_CREATE_TEMP_TRIGGER => Self::CreateTempTrigger,
            SQLITE_CREATE_TEMP_VIEW => Self::CreateTempView,
            SQLITE_CREATE_TRIGGER => Self::CreateTrigger,
            SQLITE_CREATE_VIEW => Self::CreateView,
            SQLITE_DELETE => Self::Delete,
            SQLITE_DROP_INDEX => Self::DropIndex,
            SQLITE_DROP_TABLE => Self::DropTable,
            SQLITE_DROP_TEMP_INDEX => Self::DropTempIndex,
            SQLITE_DROP_TEMP_TABLE => Self::DropTempTable,
            SQLITE_DROP_TEMP_TRIGGER => Self::DropTempTrigger,
            SQLITE_DROP_TEMP_VIEW => Self::DropTempView,
            SQLITE_DROP_TRIGGER => Self::DropTrigger,
            SQLITE_DROP_VIEW => Self::DropView,
            SQLITE_INSERT => Self::Insert,
            SQLITE_PRAGMA => Self::Pragma,
            SQLITE_READ => Self::Read,
            SQLITE_SELECT => Self::Select,
            SQLITE_TRANSACTION => Self::Transaction,
            SQLITE_UPDATE => Self::Update,
            SQLITE_ATTACH => Self::Attach,
            SQLITE_DETACH => Self::Detach,
            SQLITE_ALTER_TABLE => Self::AlterTable,
            SQLITE_REINDEX => Self::Reindex,
            SQLITE_ANALYZE => Self::Analyze,
            SQLITE_CREATE_VTABLE => Self::CreateVtable,
            SQLITE_DROP_VTABLE => Self::DropVtable,
            SQLITE_FUNCTION => Self::Function,
            SQLITE_SAVEPOINT => Self::Savepoint,
            SQLITE_RECURSIVE => Self::Recursive,
            other => Self::Unknown(other),
        }
    }
}

/// Boxed authorizer callback owned by the connection worker.
pub type AuthorizerFn = Box<
    dyn Fn(ActionCode, Option<&str>, Option<&str>, Option<&str>, Option<&str>) -> AuthResult
        + Send
        + 'static,
>;

/// Heap state shared with SQLite's `sqlite3_set_authorizer` callback.
pub struct AuthorizerState {
    /// Authorizer predicate invoked during statement preparation.
    callback: AuthorizerFn,
}

impl AuthorizerState {
    /// Wrap an authorizer callback for registration with SQLite.
    pub(crate) fn new(callback: AuthorizerFn) -> Self {
        Self { callback }
    }
}

/// Install an authorizer callback on `db`.
///
/// On success, returns the raw userdata pointer that the caller must free with
/// [`drop_authorizer`] after replacing or clearing the authorizer.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub unsafe fn set_authorizer(
    db: *mut sqlite3,
    state: AuthorizerState,
) -> Result<*mut c_void, SqliteError> {
    let ptr = Box::into_raw(Box::new(state)).cast::<c_void>();
    // SAFETY: caller guarantees `db` is a live SQLite connection handle and
    // will keep `ptr` alive until cleared or replaced.
    match unsafe { ffi::set_authorizer(db, Some(authorizer_entry), ptr) } {
        Ok(()) => Ok(ptr),
        Err(err) => {
            // SAFETY: registration failed so SQLite does not retain `ptr`.
            unsafe { drop_authorizer(ptr) };
            Err(err)
        }
    }
}

/// Clear the authorizer callback on `db`.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub unsafe fn clear_authorizer(db: *mut sqlite3) -> Result<(), SqliteError> {
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi::set_authorizer(db, None, ptr::null_mut()) }
}

/// Drop userdata returned by [`set_authorizer`].
///
/// # Safety
/// - `ptr` must be null or a pointer returned by [`set_authorizer`] that is no
///   longer registered with SQLite.
pub unsafe fn drop_authorizer(ptr: *mut c_void) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees `ptr` came from `set_authorizer` and is no
        // longer referenced by SQLite.
        drop(unsafe { Box::from_raw(ptr.cast::<AuthorizerState>()) });
    }
}

/// SQLite authorizer entry point. Panics fail closed with `SQLITE_DENY`.
unsafe extern "C" fn authorizer_entry(
    p: *mut c_void,
    action_code: c_int,
    arg1: *const c_char,
    arg2: *const c_char,
    db_name: *const c_char,
    trigger_or_view: *const c_char,
) -> c_int {
    let result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: SQLite passes the `AuthorizerState` pointer installed by
        // `set_authorizer`.
        let state = unsafe { &*p.cast::<AuthorizerState>() };
        // SAFETY: SQLite passes either null or valid NUL-terminated C strings
        // for the duration of the callback.
        let arg1 = unsafe { cstr_to_opt_cow(arg1) };
        let arg2 = unsafe { cstr_to_opt_cow(arg2) };
        let db_name = unsafe { cstr_to_opt_cow(db_name) };
        let trigger_or_view = unsafe { cstr_to_opt_cow(trigger_or_view) };
        (state.callback)(
            ActionCode::from_raw(action_code),
            arg1.as_deref(),
            arg2.as_deref(),
            db_name.as_deref(),
            trigger_or_view.as_deref(),
        )
        .as_raw()
    }));
    result.unwrap_or(SQLITE_DENY)
}

/// Borrow a nullable SQLite C string as a UTF-8 [`Cow`].
///
/// # Safety
/// - `ptr` must be null or point to a valid NUL-terminated C string.
unsafe fn cstr_to_opt_cow<'a>(ptr: *const c_char) -> Option<Cow<'a, str>> {
    if ptr.is_null() {
        None
    } else {
        // SAFETY: caller guarantees `ptr` is a valid NUL-terminated C string.
        Some(unsafe { CStr::from_ptr(ptr) }.to_string_lossy())
    }
}

/// Row-change operation reported by `sqlite3_update_hook`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    /// `INSERT`.
    Insert,
    /// `UPDATE`.
    Update,
    /// `DELETE`.
    Delete,
}

/// One row change delivered by [`crate::Connection::on_update`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateEvent {
    /// Insert, update, or delete.
    pub op: UpdateOp,
    /// Database name, usually `main`.
    pub database: String,
    /// Table name.
    pub table: String,
    /// Rowid of the changed row.
    pub rowid: i64,
}

/// State shared with a SQLite hook callback.
pub struct HookState<T> {
    /// Channel used to deliver events without blocking SQLite.
    tx: flume::Sender<T>,
    /// Count of events dropped because the receiver is gone.
    dropped: Arc<AtomicUsize>,
}

impl<T> HookState<T> {
    /// Build hook state that delivers events on `tx`.
    pub fn new(tx: flume::Sender<T>, dropped: Arc<AtomicUsize>) -> Self {
        Self { tx, dropped }
    }

    /// Deliver one event without blocking, counting drops when the receiver is
    /// gone.
    fn send(&self, event: T) {
        if self.tx.try_send(event).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Install an update hook. Returns the previous userdata pointer, if any.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub unsafe fn set_update_hook(db: *mut sqlite3, state: HookState<UpdateEvent>) -> *mut c_void {
    let ptr = Box::into_raw(Box::new(state)).cast::<c_void>();
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_update_hook(db, Some(update_entry), ptr) }
}

/// Install a commit hook that cannot veto the commit.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub unsafe fn set_commit_hook(db: *mut sqlite3, state: HookState<()>) -> *mut c_void {
    let ptr = Box::into_raw(Box::new(state)).cast::<c_void>();
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_commit_hook(db, Some(commit_entry), ptr) }
}

/// Install a rollback hook.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub unsafe fn set_rollback_hook(db: *mut sqlite3, state: HookState<()>) -> *mut c_void {
    let ptr = Box::into_raw(Box::new(state)).cast::<c_void>();
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_rollback_hook(db, Some(rollback_entry), ptr) }
}

/// Clear the update hook and return the previous userdata pointer.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub unsafe fn clear_update_hook(db: *mut sqlite3) -> *mut c_void {
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_update_hook(db, None, ptr::null_mut()) }
}

/// Clear the commit hook and return the previous userdata pointer.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub unsafe fn clear_commit_hook(db: *mut sqlite3) -> *mut c_void {
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_commit_hook(db, None, ptr::null_mut()) }
}

/// Clear the rollback hook and return the previous userdata pointer.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub unsafe fn clear_rollback_hook(db: *mut sqlite3) -> *mut c_void {
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_rollback_hook(db, None, ptr::null_mut()) }
}

/// Drop userdata returned when replacing or clearing a hook.
///
/// # Safety
/// - `ptr` must be null or a pointer created by
///   `Box::into_raw(Box::new(HookState::<T>::...))` that is no longer
///   registered with SQLite.
pub unsafe fn drop_hook<T>(ptr: *mut c_void) {
    if !ptr.is_null() {
        // SAFETY: caller guarantees `ptr` came from a boxed `HookState<T>` and
        // is exclusively owned.
        drop(unsafe { Box::from_raw(ptr.cast::<HookState<T>>()) });
    }
}

/// SQLite update-hook entry point. Copies names and `try_send`s the event.
unsafe extern "C" fn update_entry(
    p: *mut c_void,
    op: c_int,
    database: *const c_char,
    table: *const c_char,
    rowid: i64,
) {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: SQLite passes the `HookState<UpdateEvent>` pointer installed
        // by `set_update_hook`.
        let state = unsafe { &*p.cast::<HookState<UpdateEvent>>() };
        let Some(op) = update_op(op) else {
            return;
        };
        let event = UpdateEvent {
            op,
            // SAFETY: SQLite passes either null or a valid NUL-terminated C
            // string for the duration of the callback.
            database: unsafe { cstr_to_string(database) },
            // SAFETY: SQLite passes either null or a valid NUL-terminated C
            // string for the duration of the callback.
            table: unsafe { cstr_to_string(table) },
            rowid,
        };
        state.send(event);
    }))
    .ok();
}

/// SQLite commit-hook entry point. Always allows the commit.
unsafe extern "C" fn commit_entry(p: *mut c_void) -> c_int {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: SQLite passes the `HookState<()>` pointer installed by
        // `set_commit_hook`.
        unsafe { signal(p) };
    }))
    .ok();
    0
}

/// SQLite rollback-hook entry point.
unsafe extern "C" fn rollback_entry(p: *mut c_void) {
    catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: SQLite passes the `HookState<()>` pointer installed by
        // `set_rollback_hook`.
        unsafe { signal(p) };
    }))
    .ok();
}

/// Deliver a unit signal through a [`HookState<()>`].
///
/// # Safety
/// - `p` must point to a valid, live [`HookState<()>`].
unsafe fn signal(p: *mut c_void) {
    // SAFETY: caller guarantees `p` points to a live `HookState<()>`.
    let state = unsafe { &*p.cast::<HookState<()>>() };
    state.send(());
}

/// Map a SQLite update operation code.
fn update_op(op: c_int) -> Option<UpdateOp> {
    match op {
        SQLITE_INSERT => Some(UpdateOp::Insert),
        SQLITE_UPDATE => Some(UpdateOp::Update),
        SQLITE_DELETE => Some(UpdateOp::Delete),
        _ => None,
    }
}

/// Copy a SQLite C string into an owned [`String`].
unsafe fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}
