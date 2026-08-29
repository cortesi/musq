use std::{
    error::Error as StdError,
    ffi::CStr,
    fmt::{self, Display, Formatter},
};

use libsqlite3_sys::{self, sqlite3};

use crate::sqlite::ffi;

// Error Codes And Messages
// https://www.sqlite.org/c3ref/errcode.html

/// Generate a SQLite result-code enum with docs, mapping, and display names.
macro_rules! sqlite_result_codes {
    ($(#[$enum_doc:meta])* $name:ident, $($variant:ident = $const:ident : $doc:literal),+ $(,)?) => {
        $(#[$enum_doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum $name {
            $(
                #[doc = $doc]
                $variant,
            )+
            /// Unrecognized SQLite result code.
            Unknown(u32),
        }

        impl $name {
            fn from_raw(code: i32) -> Self {
                match code {
                    $(libsqlite3_sys::$const => Self::$variant,)+
                    _ => Self::Unknown(code as u32),
                }
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$variant => f.write_str(stringify!($const)),)+
                    Self::Unknown(code) => write!(f, "SQLITE_{code}"),
                }
            }
        }
    };
}

sqlite_result_codes! {
    /// Primary Sqlite error codes.
    ///
    /// **Note:** This enum is marked `#[non_exhaustive]`; avoid exhaustive
    /// matches as new variants may be introduced.
    PrimaryErrCode,
    Error = SQLITE_ERROR: "SQL error or missing database.",
    Internal = SQLITE_INTERNAL: "Internal SQLite logic error.",
    Perm = SQLITE_PERM: "Access permission denied.",
    Abort = SQLITE_ABORT: "A callback requested that the operation abort.",
    Busy = SQLITE_BUSY: "The database file is locked.",
    Locked = SQLITE_LOCKED: "A table in the database is locked.",
    NoMem = SQLITE_NOMEM: "A malloc() failed.",
    ReadOnly = SQLITE_READONLY: "Attempt to write a read-only database.",
    Interrupt = SQLITE_INTERRUPT: "Operation terminated by sqlite3_interrupt().",
    IoErr = SQLITE_IOERR: "Some kind of disk I/O error occurred.",
    Corrupt = SQLITE_CORRUPT: "The database disk image is malformed.",
    NotFound = SQLITE_NOTFOUND: "Unknown opcode in sqlite3_file_control().",
    Full = SQLITE_FULL: "Insertion failed because the database is full.",
    CantOpen = SQLITE_CANTOPEN: "Unable to open the database file.",
    Protocol = SQLITE_PROTOCOL: "Database lock protocol error.",
    Empty = SQLITE_EMPTY: "Internal use only.",
    Schema = SQLITE_SCHEMA: "The database schema changed.",
    TooBig = SQLITE_TOOBIG: "String or BLOB exceeds size limit.",
    Constraint = SQLITE_CONSTRAINT: "Abort due to constraint violation.",
    Mismatch = SQLITE_MISMATCH: "Data type mismatch.",
    Misuse = SQLITE_MISUSE: "Library used incorrectly.",
    NoLfs = SQLITE_NOLFS: "Uses OS features not supported on host.",
    Auth = SQLITE_AUTH: "Authorization denied.",
    Format = SQLITE_FORMAT: "Not used.",
    Range = SQLITE_RANGE: "2nd parameter to sqlite3_bind out of range.",
    NotADB = SQLITE_NOTADB: "File opened that is not a database file.",
    Notice = SQLITE_NOTICE: "Notifications from sqlite3_log().",
    Warning = SQLITE_WARNING: "Warnings from sqlite3_log().",
}

impl PrimaryErrCode {
    /// Convert a raw SQLite error code into a primary code.
    fn from_code(code: i32) -> Self {
        Self::from_raw(code & 255)
    }
}

sqlite_result_codes! {
    /// Extended Sqlite error codes.
    ///
    /// **Note:** This enum is marked `#[non_exhaustive]`; avoid exhaustive
    /// matches as new variants may be introduced.
    ExtendedErrCode,
    ErrorMissingCollseq = SQLITE_ERROR_MISSING_COLLSEQ: "SQL uses an unknown collating sequence.",
    ErrorRetry = SQLITE_ERROR_RETRY: "Prepare the statement again.",
    ErrorSnapshot = SQLITE_ERROR_SNAPSHOT: "The WAL snapshot is no longer valid.",
    IOErrRead = SQLITE_IOERR_READ: "I/O error while reading.",
    IOErrShortRead = SQLITE_IOERR_SHORT_READ: "I/O error: a short read.",
    IOErrWrite = SQLITE_IOERR_WRITE: "I/O error while writing.",
    IOErrFsync = SQLITE_IOERR_FSYNC: "I/O error during fsync().",
    IOErrDirFsync = SQLITE_IOERR_DIR_FSYNC: "I/O error during a directory fsync().",
    IOErrTruncate = SQLITE_IOERR_TRUNCATE: "I/O error during ftruncate().",
    IOErrFstat = SQLITE_IOERR_FSTAT: "I/O error during fstat().",
    IOErrUnlock = SQLITE_IOERR_UNLOCK: "I/O error during unlock.",
    IOErrRdlock = SQLITE_IOERR_RDLOCK: "I/O error during a read lock.",
    IOErrDelete = SQLITE_IOERR_DELETE: "I/O error during delete.",
    IOErrBlocked = SQLITE_IOERR_BLOCKED: "I/O error: lock held by another process.",
    IOErrNoMem = SQLITE_IOERR_NOMEM: "I/O error: out of memory.",
    IOErrAccess = SQLITE_IOERR_ACCESS: "I/O error during access().",
    IOErrCheckReservedLock = SQLITE_IOERR_CHECKRESERVEDLOCK: "I/O error checking a reserved lock.",
    IOErrLock = SQLITE_IOERR_LOCK: "I/O error during lock.",
    IOErrClose = SQLITE_IOERR_CLOSE: "I/O error during close().",
    IOErrDirClose = SQLITE_IOERR_DIR_CLOSE: "I/O error closing a directory.",
    IOErrShmopen = SQLITE_IOERR_SHMOPEN: "I/O error opening shared memory.",
    IOErrShmsize = SQLITE_IOERR_SHMSIZE: "I/O error setting shared-memory size.",
    IOErrShmlock = SQLITE_IOERR_SHMLOCK: "I/O error locking shared memory.",
    IOErrShmmap = SQLITE_IOERR_SHMMAP: "I/O error mapping shared memory.",
    IOErrSeek = SQLITE_IOERR_SEEK: "I/O error during seek.",
    IOErrDeleteNoent = SQLITE_IOERR_DELETE_NOENT: "I/O error: delete of a missing file.",
    IOErrMmap = SQLITE_IOERR_MMAP: "I/O error during mmap().",
    IOErrGetTempPath = SQLITE_IOERR_GETTEMPPATH: "I/O error getting a temporary path.",
    IOErrConvPath = SQLITE_IOERR_CONVPATH: "I/O error converting a file path.",
    IOErrVnode = SQLITE_IOERR_VNODE: "I/O error in a VFS vnode.",
    IOErrAuth = SQLITE_IOERR_AUTH: "I/O error from an authorization check.",
    IOErrBeginAtomic = SQLITE_IOERR_BEGIN_ATOMIC: "I/O error starting an atomic write.",
    IOErrCommitAtomic = SQLITE_IOERR_COMMIT_ATOMIC: "I/O error committing an atomic write.",
    IOErrRollbackAtomic = SQLITE_IOERR_ROLLBACK_ATOMIC: "I/O error rolling back an atomic write.",
    IOErrData = SQLITE_IOERR_DATA: "I/O error: disk content changed.",
    IOErrCorruptFs = SQLITE_IOERR_CORRUPTFS: "I/O error: the filesystem is corrupt.",
    LockedSharedCache = SQLITE_LOCKED_SHAREDCACHE: "Conflict with another connection in shared cache.",
    LockedVTab = SQLITE_LOCKED_VTAB: "A virtual table is busy.",
    BusyRecovery = SQLITE_BUSY_RECOVERY: "Another process is recovering the WAL.",
    BusySnapshot = SQLITE_BUSY_SNAPSHOT: "Cannot promote a deferred transaction.",
    BusyTimeout = SQLITE_BUSY_TIMEOUT: "Busy handler timed out.",
    CantOpenNoTempDir = SQLITE_CANTOPEN_NOTEMPDIR: "Cannot find a temporary directory.",
    CantOpenIsDir = SQLITE_CANTOPEN_ISDIR: "Attempted to open a directory.",
    CantOpenFullPath = SQLITE_CANTOPEN_FULLPATH: "Unable to obtain the full pathname.",
    CantOpenConvPath = SQLITE_CANTOPEN_CONVPATH: "Unable to convert the pathname.",
    CantOpenDirtyWal = SQLITE_CANTOPEN_DIRTYWAL: "The WAL file is leftover from a crash.",
    CantOpenSymlink = SQLITE_CANTOPEN_SYMLINK: "Symbolic links are disabled.",
    CorruptVTab = SQLITE_CORRUPT_VTAB: "Content in a virtual table is corrupt.",
    CorruptSequence = SQLITE_CORRUPT_SEQUENCE: "sqlite_sequence table is malformed.",
    CorruptIndex = SQLITE_CORRUPT_INDEX: "An index is malformed.",
    ReadOnlyRecovery = SQLITE_READONLY_RECOVERY: "WAL recovery cannot run on a read-only database.",
    ReadOnlyCantLock = SQLITE_READONLY_CANTLOCK: "Cannot take a shared lock for read-only WAL.",
    ReadOnlyRollback = SQLITE_READONLY_ROLLBACK: "Hot journal rollback is required.",
    ReadOnlyDbMoved = SQLITE_READONLY_DBMOVED: "The database file was moved.",
    ReadOnlyCantInit = SQLITE_READONLY_CANTINIT: "Cannot create a WAL or journal file.",
    ReadOnlyDirectory = SQLITE_READONLY_DIRECTORY: "The directory is not writable.",
    AbortRollback = SQLITE_ABORT_ROLLBACK: "The statement was aborted by ROLLBACK.",
    ConstraintCheck = SQLITE_CONSTRAINT_CHECK: "A CHECK constraint failed.",
    ConstraintCommitHook = SQLITE_CONSTRAINT_COMMITHOOK: "A commit hook vetoed the transaction.",
    ConstraintForeignKey = SQLITE_CONSTRAINT_FOREIGNKEY: "A foreign key constraint failed.",
    ConstraintFunction = SQLITE_CONSTRAINT_FUNCTION: "A user function reported a constraint error.",
    ConstraintNotNull = SQLITE_CONSTRAINT_NOTNULL: "A NOT NULL constraint failed.",
    ConstraintPrimaryKey = SQLITE_CONSTRAINT_PRIMARYKEY: "A PRIMARY KEY constraint failed.",
    ConstraintTrigger = SQLITE_CONSTRAINT_TRIGGER: "A RAISE(ABORT) in a trigger fired.",
    ConstraintUnique = SQLITE_CONSTRAINT_UNIQUE: "A UNIQUE constraint failed.",
    ConstraintVTab = SQLITE_CONSTRAINT_VTAB: "A virtual table constraint failed.",
    ConstraintRowId = SQLITE_CONSTRAINT_ROWID: "rowid is not unique.",
    ConstraintPinned = SQLITE_CONSTRAINT_PINNED: "The row is pinned by an incremental blob.",
    ConstraintDataType = SQLITE_CONSTRAINT_DATATYPE: "A STRICT table rejected a value.",
    NoticeRecoverWal = SQLITE_NOTICE_RECOVER_WAL: "WAL recovery recovered uncheckpointed frames.",
    NoticeRecoverRollback = SQLITE_NOTICE_RECOVER_ROLLBACK: "Hot-journal recovery rolled back a transaction.",
    WarningAutoIndex = SQLITE_WARNING_AUTOINDEX: "SQLite used an automatic index.",
    AuthUser = SQLITE_AUTH_USER: "Authorization denied for this user.",
    OkLoadPermanently = SQLITE_OK_LOAD_PERMANENTLY: "Extension loaded permanently.",
    OkSymlink = SQLITE_OK_SYMLINK: "The path is a symbolic link.",
}

impl ExtendedErrCode {
    /// Convert a raw SQLite error code into an extended code.
    fn from_code(code: i32) -> Self {
        Self::from_raw(code)
    }

    /// Returns `true` when this extended code represents a busy condition.
    pub(crate) fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::BusyRecovery | Self::BusySnapshot | Self::BusyTimeout
        )
    }

    /// Returns `true` when this code represents a unique-value conflict.
    pub(crate) fn is_unique_violation(&self) -> bool {
        matches!(
            self,
            Self::ConstraintPrimaryKey | Self::ConstraintUnique | Self::ConstraintRowId
        )
    }
}

/// An error returned from SQLite.
#[derive(Debug, Clone)]
pub struct SqliteError {
    /// Primary error code.
    pub primary: PrimaryErrCode,
    /// Extended error code. `None` when SQLite reported only the primary code.
    pub extended: Option<ExtendedErrCode>,
    /// SQLite-provided error message.
    pub message: String,
    /// Byte offset of the failing token in the SQL, when SQLite reports one.
    pub offset: Option<usize>,
}

impl SqliteError {
    /// Build a new error from a raw SQLite result code and message.
    pub(crate) fn from_code(code: i32, message: impl Into<String>) -> Self {
        let primary = PrimaryErrCode::from_code(code);
        let extended = if code & 255 == code {
            None
        } else {
            Some(ExtendedErrCode::from_code(code))
        };
        Self {
            primary,
            extended,
            message: message.into(),
            offset: None,
        }
    }

    /// Build a new error from the active SQLite handle.
    pub(crate) fn new(handle: *mut sqlite3) -> Self {
        // SAFETY: the caller provided a live connection handle.
        let code = unsafe { ffi::extended_errcode(handle) };
        let message = unsafe {
            let msg = ffi::errmsg(handle);
            debug_assert!(!msg.is_null());
            CStr::from_ptr(msg).to_string_lossy().into_owned()
        };

        Self {
            offset: unsafe { ffi::error_offset(handle) },
            ..Self::from_code(code, message)
        }
    }

    /// Returns `true` if the error represents a busy condition.
    pub fn is_busy(&self) -> bool {
        self.primary == PrimaryErrCode::Busy || self.extended.is_some_and(|code| code.is_busy())
    }

    /// Returns `true` if the error represents a unique-value conflict.
    pub fn is_unique_violation(&self) -> bool {
        self.extended.is_some_and(|code| code.is_unique_violation())
    }

    /// Return the primary and extended SQLite error codes.
    pub fn codes(&self) -> (PrimaryErrCode, Option<ExtendedErrCode>) {
        (self.primary, self.extended)
    }
}

impl Display for SqliteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.primary, f)?;
        if let Some(extended) = self.extended {
            write!(f, " ({extended})")?;
        }
        if let Some(offset) = self.offset {
            write!(f, " at byte {offset}")?;
        }
        write!(f, ": {}", self.message)
    }
}

impl StdError for SqliteError {}
