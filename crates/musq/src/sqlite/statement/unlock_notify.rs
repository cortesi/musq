use std::ffi::c_void;

use std::slice;
use std::sync::{Condvar, Mutex};

use crate::sqlite::ffi;
use libsqlite3_sys::{SQLITE_LOCKED, sqlite3, sqlite3_stmt};

use crate::{
    error::{Error, Result},
    sqlite::error::{ExtendedErrCode, PrimaryErrCode, SqliteError},
};

/// Default number of times [`wait`] will retry when a statement is reset due to
/// `SQLITE_LOCKED`.
pub const DEFAULT_MAX_RETRIES: usize = 5;

// Wait for unlock notification (https://www.sqlite.org/unlock_notify.html)
// If `stmt` is provided, it will be reset and the call retried when
// `SQLITE_LOCKED` is returned. The `max_retries` parameter controls how many
// times to retry this reset before giving up.
pub fn wait(conn: *mut sqlite3, stmt: Option<*mut sqlite3_stmt>, max_retries: usize) -> Result<()> {
    let notify = Notify::new();
    let mut attempts = 0;
    loop {
        match ffi::unlock_notify(
            conn,
            Some(unlock_notify_cb),
            &notify as *const Notify as *mut Notify as *mut _,
        ) {
            Ok(()) => {}
            Err(e) if e.primary == PrimaryErrCode::Locked => {
                if let Some(stmt) = stmt {
                    let _ = ffi::reset(stmt);
                    attempts += 1;

                    if attempts > max_retries {
                        return Err(Error::UnlockNotify);
                    }

                    continue;
                }

                // See https://www.sqlite.org/unlock_notify.html. SQLITE_LOCKED indicates
                // that a deadlock was detected and the unlock notification was not
                // queued. The statement should be reset or stepped to break the
                // deadlock before retrying.
                return Err(Error::Sqlite(SqliteError {
                    primary: PrimaryErrCode::Locked,
                    extended: ExtendedErrCode::Unknown(SQLITE_LOCKED as u32),
                    message:
                        "sqlite3_unlock_notify returned SQLITE_LOCKED (deadlock). Reset the blocking statement and retry".to_string(),
                }));
            }
            Err(e) => return Err(Error::Sqlite(e)),
        }

        break;
    }

    notify.wait();

    Ok(())
}

unsafe extern "C" fn unlock_notify_cb(ptr: *mut *mut c_void, len: i32) {
    let ptr = ptr as *mut *mut Notify;
    let slice = unsafe { slice::from_raw_parts(ptr, len as usize) };

    for &notify_ptr in slice {
        unsafe { (&*notify_ptr).fire() };
    }
}

struct Notify {
    mutex: Mutex<bool>,
    condvar: Condvar,
}

impl Notify {
    fn new() -> Self {
        Self {
            mutex: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    fn wait(&self) {
        // We only want to wait until the lock is available again.
        #[allow(let_underscore_lock)]
        let _ = self
            .condvar
            .wait_while(self.mutex.lock().unwrap(), |fired| !*fired)
            .unwrap();
    }

    fn fire(&self) {
        let mut lock = self.mutex.lock().unwrap();
        *lock = true;
        self.condvar.notify_one();
    }
}
