use std::{ffi::CString, ptr::NonNull};

use libsqlite3_sys::sqlite3;

use crate::{Error, Result, sqlite::ffi};

/// Managed handle to the raw SQLite3 database handle.
/// The database handle will be closed when this is dropped and no
/// `ConnectionHandleRef`s exist.
#[derive(Debug)]
pub struct ConnectionHandle {
    /// Raw SQLite pointer.
    ptr: NonNull<sqlite3>,
    /// Whether the handle has been closed.
    closed: bool,
}

// A SQLite3 handle is safe to send between threads, provided not more than
// one is accessing it at the same time. This is upheld as long as
// [SQLITE_CONFIG_MULTITHREAD] is enabled and [SQLITE_THREADSAFE] was enabled
// when sqlite was compiled. We refuse to work if these conditions are not
// upheld.

// <https://www.sqlite.org/c3ref/threadsafe.html>

// <https://www.sqlite.org/c3ref/c_config_covering_index_scan.html#sqliteconfigmultithread>

unsafe impl Send for ConnectionHandle {}

impl ConnectionHandle {
    /// Construct a new handle from a raw SQLite pointer.
    pub(super) unsafe fn new(ptr: *mut sqlite3) -> Self {
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            closed: false,
        }
    }

    /// Return the raw SQLite pointer.
    pub(crate) fn as_ptr(&self) -> *mut sqlite3 {
        self.ptr.as_ptr()
    }

    /// Return the SQLite pointer as [`NonNull`].
    pub(crate) fn as_non_null(&self) -> NonNull<sqlite3> {
        self.ptr
    }

    /// Return the last inserted row id for this connection.
    pub(crate) fn last_insert_rowid(&self) -> i64 {
        // SAFETY: this handle owns a live connection.
        unsafe { ffi::last_insert_rowid(self.as_ptr()) }
    }

    /// Execute a SQL statement without returning rows.
    pub(crate) fn exec(&self, query: impl Into<String>) -> Result<()> {
        let query = query.into();
        let query =
            CString::new(query).map_err(|_| Error::Query("query contains nul bytes".into()))?;

        // SAFETY: this handle owns a live connection; `query` is a valid CString.
        unsafe { ffi::exec(self.as_ptr(), query.as_ptr()) }.map_err(Error::from)
    }

    /// Close the underlying SQLite handle.
    pub(crate) fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        // SAFETY: this handle owns a live connection until close succeeds.
        match unsafe { ffi::close(self.ptr.as_ptr()) } {
            Ok(()) => {
                self.closed = true;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Drop for ConnectionHandle {
    fn drop(&mut self) {
        // https://sqlite.org/c3ref/close.html
        if !self.closed
            && let Err(e) = unsafe { ffi::close(self.ptr.as_ptr()) }
        {
            // This should only happen if SQLite has leaked handles internally
            // or we misused the API. Log the error and the connection pointer
            // so that we can troubleshoot the issue if it happens in the wild.
            tracing::error!(db_ptr = ?self.ptr, "sqlite3_close failed: {}", e);
        }
    }
}
