use std::error::Error as StdError;
use std::ffi::CStr;
use std::fmt::{self, Display, Formatter};
use std::os::raw::c_int;
use std::{borrow::Cow, str::from_utf8_unchecked};

use libsqlite3_sys::{
    sqlite3, sqlite3_errmsg, sqlite3_extended_errcode, SQLITE_CONSTRAINT_CHECK,
    SQLITE_CONSTRAINT_FOREIGNKEY, SQLITE_CONSTRAINT_NOTNULL, SQLITE_CONSTRAINT_PRIMARYKEY,
    SQLITE_CONSTRAINT_UNIQUE,
};

pub(crate) use crate::error::*;

// Error Codes And Messages
// https://www.sqlite.org/c3ref/errcode.html

#[derive(Debug)]
pub struct SqliteError {
    code: c_int,
    message: String,
}

impl SqliteError {
    pub(crate) fn new(handle: *mut sqlite3) -> Self {
        // returns the extended result code even when extended result codes are disabled
        let code: c_int = unsafe { sqlite3_extended_errcode(handle) };

        // return English-language text that describes the error
        let message = unsafe {
            let msg = sqlite3_errmsg(handle);
            debug_assert!(!msg.is_null());

            from_utf8_unchecked(CStr::from_ptr(msg).to_bytes())
        };

        Self {
            code,
            message: message.to_owned(),
        }
    }

    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The extended result code.
    #[inline]
    pub fn code(&self) -> Option<Cow<'_, str>> {
        Some(format!("{}", self.code).into())
    }

    #[doc(hidden)]
    pub fn as_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
        self
    }

    #[doc(hidden)]
    pub fn as_error_mut(&mut self) -> &mut (dyn StdError + Send + Sync + 'static) {
        self
    }

    pub fn kind(&self) -> ErrorKind {
        match self.code {
            SQLITE_CONSTRAINT_UNIQUE | SQLITE_CONSTRAINT_PRIMARYKEY => ErrorKind::UniqueViolation,
            SQLITE_CONSTRAINT_FOREIGNKEY => ErrorKind::ForeignKeyViolation,
            SQLITE_CONSTRAINT_NOTNULL => ErrorKind::NotNullViolation,
            SQLITE_CONSTRAINT_CHECK => ErrorKind::CheckViolation,
            _ => ErrorKind::Other,
        }
    }
}

impl Display for SqliteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // We include the code as some produce ambiguous messages:
        // SQLITE_BUSY: "database is locked"
        // SQLITE_LOCKED: "database table is locked"
        // Sadly there's no function to get the string label back from an error code.
        write!(f, "(code: {}) {}", self.code, self.message)
    }
}

impl StdError for SqliteError {}
