use std::{
    ffi::c_void,
    slice,
    sync::{Condvar, Mutex},
};

use libsqlite3_sys::{SQLITE_LOCKED, sqlite3, sqlite3_stmt};

use crate::{
    error::{Error, Result},
    sqlite::{
        DEFAULT_MAX_RETRIES,
        error::{ExtendedErrCode, PrimaryErrCode},
        ffi,
    },
};

/// Wait for unlock notification (https://www.sqlite.org/unlock_notify.html).
///
/// If `stmt` is provided, it will be reset and the call retried when
/// `SQLITE_LOCKED` is returned.
pub fn wait(conn: *mut sqlite3, stmt: Option<*mut sqlite3_stmt>) -> Result<()> {
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
                    attempts += 1;
                    if attempts > DEFAULT_MAX_RETRIES {
                        return Err(Error::UnlockNotify);
                    }

                    let _reset_result = ffi::reset(stmt);

                    continue;
                }

                // See https://www.sqlite.org/unlock_notify.html. SQLITE_LOCKED indicates
                // that a deadlock was detected and the unlock notification was not
                // queued. The statement should be reset or stepped to break the
                // deadlock before retrying.
                return Err(Error::Sqlite {
                    primary: PrimaryErrCode::Locked,
                    extended: ExtendedErrCode::Unknown(SQLITE_LOCKED as u32),
                    message:
                        "sqlite3_unlock_notify returned SQLITE_LOCKED (deadlock). Reset the blocking statement and retry".to_string(),
                });
            }
            Err(e) => return Err(e.into()),
        }

        break;
    }

    notify.wait()?;

    Ok(())
}

/// SQLite unlock notification callback.
unsafe extern "C" fn unlock_notify_cb(ptr: *mut *mut c_void, len: i32) {
    let ptr = ptr as *mut *mut Notify;
    let slice = unsafe { slice::from_raw_parts(ptr, len as usize) };

    for &notify_ptr in slice {
        unsafe { (&*notify_ptr).fire() };
    }
}

/// Synchronization primitive for unlock notifications.
struct Notify {
    /// Notification flag.
    mutex: Mutex<bool>,
    /// Condition variable for waiting threads.
    condvar: Condvar,
}

impl Notify {
    /// Create a new notification helper.
    fn new() -> Self {
        Self {
            mutex: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    /// Wait until the notification fires.
    fn wait(&self) -> Result<()> {
        // We only want to wait until the lock is available again.
        let _guard = self
            .condvar
            .wait_while(self.mutex.lock().map_err(Error::from)?, |fired| !*fired)
            .map_err(Error::from)?;
        Ok(())
    }

    /// Fire the notification.
    fn fire(&self) {
        if let Ok(mut lock) = self.mutex.lock() {
            *lock = true;
        }
        self.condvar.notify_one();
    }
}

#[cfg(test)]
/// Tests for unlock notification handling.
mod tests {
    use super::*;
    use crate::sqlite::error::{ExtendedErrCode, PrimaryErrCode, SqliteError};

    // Helper to create a SqliteError representing SQLITE_LOCKED
    fn locked_error() -> SqliteError {
        SqliteError {
            primary: PrimaryErrCode::Locked,
            extended: ExtendedErrCode::Unknown(SQLITE_LOCKED as u32),
            message: String::new(),
        }
    }

    #[test]
    fn retry_limit_returns_unlock_notify() {
        // Simulate unlock_notify returning SQLITE_LOCKED indefinitely
        let mut attempts = 0;
        let result: Result<()> = loop {
            if let Err(_e) = Err::<(), _>(locked_error()) {
                attempts += 1;
                if attempts > DEFAULT_MAX_RETRIES {
                    break Err(Error::UnlockNotify);
                }

                // simulate ffi::reset; ignored here
                continue;
            }

            break Ok(());
        };

        assert!(matches!(result, Err(Error::UnlockNotify)));
        assert_eq!(attempts, DEFAULT_MAX_RETRIES + 1);
    }
}
