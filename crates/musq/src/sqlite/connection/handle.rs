use std::{ffi::CString, ptr::NonNull};

use libsqlite3_sys::sqlite3;

use crate::sqlite::ffi;

use crate::{
    sqlite::{statement::unlock_notify, error::ExtendedErrCode},
    Error,
};

/// Managed handle to the raw SQLite3 database handle.
/// The database handle will be closed when this is dropped and no `ConnectionHandleRef`s exist.
#[derive(Debug)]
pub(crate) struct ConnectionHandle(NonNull<sqlite3>);

// A SQLite3 handle is safe to send between threads, provided not more than
// one is accessing it at the same time. This is upheld as long as [SQLITE_CONFIG_MULTITHREAD] is
// enabled and [SQLITE_THREADSAFE] was enabled when sqlite was compiled. We refuse to work
// if these conditions are not upheld.

// <https://www.sqlite.org/c3ref/threadsafe.html>

// <https://www.sqlite.org/c3ref/c_config_covering_index_scan.html#sqliteconfigmultithread>

unsafe impl Send for ConnectionHandle {}

impl ConnectionHandle {
    pub(super) unsafe fn new(ptr: *mut sqlite3) -> Self {
        Self(unsafe { NonNull::new_unchecked(ptr) })
    }

    pub(crate) fn as_ptr(&self) -> *mut sqlite3 {
        self.0.as_ptr()
    }

    pub(crate) fn as_non_null_ptr(&self) -> NonNull<sqlite3> {
        self.0
    }

    pub(crate) fn last_insert_rowid(&self) -> i64 {
        // SAFETY: we have exclusive access to the database handle
        ffi::last_insert_rowid(self.as_ptr())
    }

    pub(crate) fn exec(&self, query: impl Into<String>) -> Result<(), Error> {
        let query = query.into();
        let query =
            CString::new(query).map_err(|_| Error::Protocol("query contains nul bytes".into()))?;

        // SAFETY: we have exclusive access to the database handle
        loop {
            match ffi::exec(self.as_ptr(), query.as_ptr()) {
                Ok(()) => return Ok(()),
                Err(e) if e.extended == ExtendedErrCode::LockedSharedCache => {
                    unlock_notify::wait(self.as_ptr(), None)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl Drop for ConnectionHandle {
    fn drop(&mut self) {
        // https://sqlite.org/c3ref/close.html
        if let Err(e) = ffi::close(self.0.as_ptr()) {
            // this should *only* happen due to an internal bug in SQLite where we left
            // SQLite handles open
            panic!("{}", e);
        }
    }
}
