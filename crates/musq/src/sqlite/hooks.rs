//! Per-connection SQLite update, commit, and rollback hooks.

use std::{
    ffi::{CStr, c_void},
    os::raw::{c_char, c_int},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use libsqlite3_sys::{self as ffi_sys, SQLITE_DELETE, SQLITE_INSERT, SQLITE_UPDATE, sqlite3};

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
pub(crate) struct HookState<T> {
    /// Channel used to deliver events without blocking SQLite.
    tx: flume::Sender<T>,
    /// Count of events dropped because the receiver is gone.
    dropped: Arc<AtomicUsize>,
}

impl<T> HookState<T> {
    /// Build hook state that delivers events on `tx`.
    pub(crate) fn new(tx: flume::Sender<T>, dropped: Arc<AtomicUsize>) -> Self {
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
pub(crate) unsafe fn set_update_hook(
    db: *mut sqlite3,
    state: HookState<UpdateEvent>,
) -> *mut c_void {
    let ptr = Box::into_raw(Box::new(state)).cast::<c_void>();
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_update_hook(db, Some(update_entry), ptr) }
}

/// Install a commit hook that cannot veto the commit.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub(crate) unsafe fn set_commit_hook(db: *mut sqlite3, state: HookState<()>) -> *mut c_void {
    let ptr = Box::into_raw(Box::new(state)).cast::<c_void>();
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_commit_hook(db, Some(commit_entry), ptr) }
}

/// Install a rollback hook.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub(crate) unsafe fn set_rollback_hook(db: *mut sqlite3, state: HookState<()>) -> *mut c_void {
    let ptr = Box::into_raw(Box::new(state)).cast::<c_void>();
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_rollback_hook(db, Some(rollback_entry), ptr) }
}

/// Clear the update hook and return the previous userdata pointer.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub(crate) unsafe fn clear_update_hook(db: *mut sqlite3) -> *mut c_void {
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_update_hook(db, None, ptr::null_mut()) }
}

/// Clear the commit hook and return the previous userdata pointer.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub(crate) unsafe fn clear_commit_hook(db: *mut sqlite3) -> *mut c_void {
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_commit_hook(db, None, ptr::null_mut()) }
}

/// Clear the rollback hook and return the previous userdata pointer.
///
/// # Safety
/// - `db` must be a valid pointer to an open SQLite connection.
pub(crate) unsafe fn clear_rollback_hook(db: *mut sqlite3) -> *mut c_void {
    // SAFETY: caller guarantees `db` is a live SQLite connection handle.
    unsafe { ffi_sys::sqlite3_rollback_hook(db, None, ptr::null_mut()) }
}

/// Drop userdata returned when replacing or clearing a hook.
///
/// # Safety
/// - `ptr` must be null or a pointer created by
///   `Box::into_raw(Box::new(HookState::<T>::...))` that is no longer
///   registered with SQLite.
pub(crate) unsafe fn drop_hook<T>(ptr: *mut c_void) {
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
        let state = unsafe { &*p.cast::<HookState<UpdateEvent>>() };
        let Some(op) = update_op(op) else {
            return;
        };
        let event = UpdateEvent {
            op,
            database: unsafe { cstr_to_string(database) },
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
