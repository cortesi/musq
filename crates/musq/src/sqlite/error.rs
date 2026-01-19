use std::ffi::CStr;

use libsqlite3_sys::{self, sqlite3};

use crate::sqlite::ffi;

// Error Codes And Messages
// https://www.sqlite.org/c3ref/errcode.html

/// Primary Sqlite error codes.
///
/// **Note:** This enum is marked `#[non_exhaustive]`; avoid exhaustive
/// matches as new variants may be introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrimaryErrCode {
    /// SQLite error code variant.
    Error,
    /// SQLite error code variant.
    Internal,
    /// SQLite error code variant.
    Perm,
    /// SQLite error code variant.
    Abort,
    /// SQLite error code variant.
    Busy,
    /// SQLite error code variant.
    Locked,
    /// SQLite error code variant.
    NoMem,
    /// SQLite error code variant.
    ReadOnly,
    /// SQLite error code variant.
    Interrupt,
    /// SQLite error code variant.
    IoErr,
    /// SQLite error code variant.
    Corrupt,
    /// SQLite error code variant.
    NotFound,
    /// SQLite error code variant.
    Full,
    /// SQLite error code variant.
    CantOpen,
    /// SQLite error code variant.
    Protocol,
    /// SQLite error code variant.
    Empty,
    /// SQLite error code variant.
    Schema,
    /// SQLite error code variant.
    TooBig,
    /// SQLite error code variant.
    Constraint,
    /// SQLite error code variant.
    Mismatch,
    /// SQLite error code variant.
    Misuse,
    /// SQLite error code variant.
    NoLfs,
    /// SQLite error code variant.
    Auth,
    /// SQLite error code variant.
    Format,
    /// SQLite error code variant.
    Range,
    /// SQLite error code variant.
    NotADB,
    /// SQLite error code variant.
    Notice,
    /// SQLite error code variant.
    Warning,
    /// SQLite error code variant.
    Unknown(u32),
}

impl PrimaryErrCode {
    /// Convert a raw SQLite error code into a primary code.
    fn from_code(code: i32) -> Self {
        match code & 255 {
            libsqlite3_sys::SQLITE_ERROR => Self::Error,
            libsqlite3_sys::SQLITE_INTERNAL => Self::Internal,
            libsqlite3_sys::SQLITE_PERM => Self::Perm,
            libsqlite3_sys::SQLITE_ABORT => Self::Abort,
            libsqlite3_sys::SQLITE_BUSY => Self::Busy,
            libsqlite3_sys::SQLITE_LOCKED => Self::Locked,
            libsqlite3_sys::SQLITE_NOMEM => Self::NoMem,
            libsqlite3_sys::SQLITE_READONLY => Self::ReadOnly,
            libsqlite3_sys::SQLITE_INTERRUPT => Self::Interrupt,
            libsqlite3_sys::SQLITE_IOERR => Self::IoErr,
            libsqlite3_sys::SQLITE_CORRUPT => Self::Corrupt,
            libsqlite3_sys::SQLITE_NOTFOUND => Self::NotFound,
            libsqlite3_sys::SQLITE_FULL => Self::Full,
            libsqlite3_sys::SQLITE_CANTOPEN => Self::CantOpen,
            libsqlite3_sys::SQLITE_PROTOCOL => Self::Protocol,
            libsqlite3_sys::SQLITE_EMPTY => Self::Empty,
            libsqlite3_sys::SQLITE_SCHEMA => Self::Schema,
            libsqlite3_sys::SQLITE_TOOBIG => Self::TooBig,
            libsqlite3_sys::SQLITE_CONSTRAINT => Self::Constraint,
            libsqlite3_sys::SQLITE_MISMATCH => Self::Mismatch,
            libsqlite3_sys::SQLITE_MISUSE => Self::Misuse,
            libsqlite3_sys::SQLITE_NOLFS => Self::NoLfs,
            libsqlite3_sys::SQLITE_AUTH => Self::Auth,
            libsqlite3_sys::SQLITE_FORMAT => Self::Format,
            libsqlite3_sys::SQLITE_RANGE => Self::Range,
            libsqlite3_sys::SQLITE_NOTADB => Self::NotADB,
            libsqlite3_sys::SQLITE_NOTICE => Self::Notice,
            libsqlite3_sys::SQLITE_WARNING => Self::Warning,
            _ => Self::Unknown(code as u32),
        }
    }
}

/// Extended Sqlite error codes.
///
/// **Note:** This enum is marked `#[non_exhaustive]`; avoid exhaustive
/// matches as new variants may be introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExtendedErrCode {
    /// SQLite error code variant.
    ErrorMissingCollseq,
    /// SQLite error code variant.
    ErrorRetry,
    /// SQLite error code variant.
    ErrorSnapshot,
    /// SQLite error code variant.
    IOErrRead,
    /// SQLite error code variant.
    IOErrShortRead,
    /// SQLite error code variant.
    IOErrWrite,
    /// SQLite error code variant.
    IOErrFsync,
    /// SQLite error code variant.
    IOErrDirFsync,
    /// SQLite error code variant.
    IOErrTruncate,
    /// SQLite error code variant.
    IOErrFstat,
    /// SQLite error code variant.
    IOErrUnlock,
    /// SQLite error code variant.
    IOErrRdlock,
    /// SQLite error code variant.
    IOErrDelete,
    /// SQLite error code variant.
    IOErrBlocked,
    /// SQLite error code variant.
    IOErrNoMem,
    /// SQLite error code variant.
    IOErrAccess,
    /// SQLite error code variant.
    IOErrCheckReservedLock,
    /// SQLite error code variant.
    IOErrLock,
    /// SQLite error code variant.
    IOErrClose,
    /// SQLite error code variant.
    IOErrDirClose,
    /// SQLite error code variant.
    IOErrShmopen,
    /// SQLite error code variant.
    IOErrShmsize,
    /// SQLite error code variant.
    IOErrShmlock,
    /// SQLite error code variant.
    IOErrShmmap,
    /// SQLite error code variant.
    IOErrSeek,
    /// SQLite error code variant.
    IOErrDeleteNoent,
    /// SQLite error code variant.
    IOErrMmap,
    /// SQLite error code variant.
    IOErrGetTempPath,
    /// SQLite error code variant.
    IOErrConvPath,
    /// SQLite error code variant.
    IOErrVnode,
    /// SQLite error code variant.
    IOErrAuth,
    /// SQLite error code variant.
    IOErrBeginAtomic,
    /// SQLite error code variant.
    IOErrCommitAtomic,
    /// SQLite error code variant.
    IOErrRollbackAtomic,
    /// SQLite error code variant.
    IOErrData,
    /// SQLite error code variant.
    IOErrCorruptFs,
    /// SQLite error code variant.
    LockedSharedCache,
    /// SQLite error code variant.
    LockedVTab,
    /// SQLite error code variant.
    BusyRecovery,
    /// SQLite error code variant.
    BusySnapshot,
    /// SQLite error code variant.
    BusyTimeout,
    /// SQLite error code variant.
    CantOpenNoTempDir,
    /// SQLite error code variant.
    CantOpenIsDir,
    /// SQLite error code variant.
    CantOpenFullPath,
    /// SQLite error code variant.
    CantOpenConvPath,
    /// SQLite error code variant.
    CantOpenDirtyWal,
    /// SQLite error code variant.
    CantOpenSymlink,
    /// SQLite error code variant.
    CorruptVTab,
    /// SQLite error code variant.
    CorruptSequence,
    /// SQLite error code variant.
    CorruptIndex,
    /// SQLite error code variant.
    ReadOnlyRecovery,
    /// SQLite error code variant.
    ReadOnlyCantLock,
    /// SQLite error code variant.
    ReadOnlyRollback,
    /// SQLite error code variant.
    ReadOnlyDbMoved,
    /// SQLite error code variant.
    ReadOnlyCantInit,
    /// SQLite error code variant.
    ReadOnlyDirectory,
    /// SQLite error code variant.
    AbortRollback,
    /// SQLite error code variant.
    ConstraintCheck,
    /// SQLite error code variant.
    ConstraintCommitHook,
    /// SQLite error code variant.
    ConstraintForeignKey,
    /// SQLite error code variant.
    ConstraintFunction,
    /// SQLite error code variant.
    ConstraintNotNull,
    /// SQLite error code variant.
    ConstraintPrimaryKey,
    /// SQLite error code variant.
    ConstraintTrigger,
    /// SQLite error code variant.
    ConstraintUnique,
    /// SQLite error code variant.
    ConstraintVTab,
    /// SQLite error code variant.
    ConstraintRowId,
    /// SQLite error code variant.
    ConstraintPinned,
    /// SQLite error code variant.
    ConstraintDataType,
    /// SQLite error code variant.
    NoticeRecoverWal,
    /// SQLite error code variant.
    NoticeRecoverRollback,
    /// SQLite error code variant.
    WarningAutoIndex,
    /// SQLite error code variant.
    AuthUser,
    /// SQLite error code variant.
    OkLoadPermanently,
    /// SQLite error code variant.
    OkSymlink,
    /// SQLite error code variant.
    Unknown(u32),
}

impl ExtendedErrCode {
    /// Convert a raw SQLite error code into an extended code.
    fn from_code(code: i32) -> Self {
        match code {
            libsqlite3_sys::SQLITE_ERROR_MISSING_COLLSEQ => Self::ErrorMissingCollseq,
            libsqlite3_sys::SQLITE_ERROR_RETRY => Self::ErrorRetry,
            libsqlite3_sys::SQLITE_ERROR_SNAPSHOT => Self::ErrorSnapshot,
            libsqlite3_sys::SQLITE_IOERR_READ => Self::IOErrRead,
            libsqlite3_sys::SQLITE_IOERR_SHORT_READ => Self::IOErrShortRead,
            libsqlite3_sys::SQLITE_IOERR_WRITE => Self::IOErrWrite,
            libsqlite3_sys::SQLITE_IOERR_FSYNC => Self::IOErrFsync,
            libsqlite3_sys::SQLITE_IOERR_DIR_FSYNC => Self::IOErrDirFsync,
            libsqlite3_sys::SQLITE_IOERR_TRUNCATE => Self::IOErrTruncate,
            libsqlite3_sys::SQLITE_IOERR_FSTAT => Self::IOErrFstat,
            libsqlite3_sys::SQLITE_IOERR_UNLOCK => Self::IOErrUnlock,
            libsqlite3_sys::SQLITE_IOERR_RDLOCK => Self::IOErrRdlock,
            libsqlite3_sys::SQLITE_IOERR_DELETE => Self::IOErrDelete,
            libsqlite3_sys::SQLITE_IOERR_BLOCKED => Self::IOErrBlocked,
            libsqlite3_sys::SQLITE_IOERR_NOMEM => Self::IOErrNoMem,
            libsqlite3_sys::SQLITE_IOERR_ACCESS => Self::IOErrAccess,
            libsqlite3_sys::SQLITE_IOERR_CHECKRESERVEDLOCK => Self::IOErrCheckReservedLock,
            libsqlite3_sys::SQLITE_IOERR_LOCK => Self::IOErrLock,
            libsqlite3_sys::SQLITE_IOERR_CLOSE => Self::IOErrClose,
            libsqlite3_sys::SQLITE_IOERR_DIR_CLOSE => Self::IOErrDirClose,
            libsqlite3_sys::SQLITE_IOERR_SHMOPEN => Self::IOErrShmopen,
            libsqlite3_sys::SQLITE_IOERR_SHMSIZE => Self::IOErrShmsize,
            libsqlite3_sys::SQLITE_IOERR_SHMLOCK => Self::IOErrShmlock,
            libsqlite3_sys::SQLITE_IOERR_SHMMAP => Self::IOErrShmmap,
            libsqlite3_sys::SQLITE_IOERR_SEEK => Self::IOErrSeek,
            libsqlite3_sys::SQLITE_IOERR_DELETE_NOENT => Self::IOErrDeleteNoent,
            libsqlite3_sys::SQLITE_IOERR_MMAP => Self::IOErrMmap,
            libsqlite3_sys::SQLITE_IOERR_GETTEMPPATH => Self::IOErrGetTempPath,
            libsqlite3_sys::SQLITE_IOERR_CONVPATH => Self::IOErrConvPath,
            libsqlite3_sys::SQLITE_IOERR_VNODE => Self::IOErrVnode,
            libsqlite3_sys::SQLITE_IOERR_AUTH => Self::IOErrAuth,
            libsqlite3_sys::SQLITE_IOERR_BEGIN_ATOMIC => Self::IOErrBeginAtomic,
            libsqlite3_sys::SQLITE_IOERR_COMMIT_ATOMIC => Self::IOErrCommitAtomic,
            libsqlite3_sys::SQLITE_IOERR_ROLLBACK_ATOMIC => Self::IOErrRollbackAtomic,
            libsqlite3_sys::SQLITE_IOERR_DATA => Self::IOErrData,
            libsqlite3_sys::SQLITE_IOERR_CORRUPTFS => Self::IOErrCorruptFs,
            libsqlite3_sys::SQLITE_LOCKED_SHAREDCACHE => Self::LockedSharedCache,
            libsqlite3_sys::SQLITE_LOCKED_VTAB => Self::LockedVTab,
            libsqlite3_sys::SQLITE_BUSY_RECOVERY => Self::BusyRecovery,
            libsqlite3_sys::SQLITE_BUSY_SNAPSHOT => Self::BusySnapshot,
            libsqlite3_sys::SQLITE_BUSY_TIMEOUT => Self::BusyTimeout,
            libsqlite3_sys::SQLITE_CANTOPEN_NOTEMPDIR => Self::CantOpenNoTempDir,
            libsqlite3_sys::SQLITE_CANTOPEN_ISDIR => Self::CantOpenIsDir,
            libsqlite3_sys::SQLITE_CANTOPEN_FULLPATH => Self::CantOpenFullPath,
            libsqlite3_sys::SQLITE_CANTOPEN_CONVPATH => Self::CantOpenConvPath,
            libsqlite3_sys::SQLITE_CANTOPEN_DIRTYWAL => Self::CantOpenDirtyWal,
            libsqlite3_sys::SQLITE_CANTOPEN_SYMLINK => Self::CantOpenSymlink,
            libsqlite3_sys::SQLITE_CORRUPT_VTAB => Self::CorruptVTab,
            libsqlite3_sys::SQLITE_CORRUPT_SEQUENCE => Self::CorruptSequence,
            libsqlite3_sys::SQLITE_CORRUPT_INDEX => Self::CorruptIndex,
            libsqlite3_sys::SQLITE_READONLY_RECOVERY => Self::ReadOnlyRecovery,
            libsqlite3_sys::SQLITE_READONLY_CANTLOCK => Self::ReadOnlyCantLock,
            libsqlite3_sys::SQLITE_READONLY_ROLLBACK => Self::ReadOnlyRollback,
            libsqlite3_sys::SQLITE_READONLY_DBMOVED => Self::ReadOnlyDbMoved,
            libsqlite3_sys::SQLITE_READONLY_CANTINIT => Self::ReadOnlyCantInit,
            libsqlite3_sys::SQLITE_READONLY_DIRECTORY => Self::ReadOnlyDirectory,
            libsqlite3_sys::SQLITE_ABORT_ROLLBACK => Self::AbortRollback,
            libsqlite3_sys::SQLITE_CONSTRAINT_CHECK => Self::ConstraintCheck,
            libsqlite3_sys::SQLITE_CONSTRAINT_COMMITHOOK => Self::ConstraintCommitHook,
            libsqlite3_sys::SQLITE_CONSTRAINT_FOREIGNKEY => Self::ConstraintForeignKey,
            libsqlite3_sys::SQLITE_CONSTRAINT_FUNCTION => Self::ConstraintFunction,
            libsqlite3_sys::SQLITE_CONSTRAINT_NOTNULL => Self::ConstraintNotNull,
            libsqlite3_sys::SQLITE_CONSTRAINT_PRIMARYKEY => Self::ConstraintPrimaryKey,
            libsqlite3_sys::SQLITE_CONSTRAINT_TRIGGER => Self::ConstraintTrigger,
            libsqlite3_sys::SQLITE_CONSTRAINT_UNIQUE => Self::ConstraintUnique,
            libsqlite3_sys::SQLITE_CONSTRAINT_VTAB => Self::ConstraintVTab,
            libsqlite3_sys::SQLITE_CONSTRAINT_ROWID => Self::ConstraintRowId,
            libsqlite3_sys::SQLITE_CONSTRAINT_PINNED => Self::ConstraintPinned,
            libsqlite3_sys::SQLITE_CONSTRAINT_DATATYPE => Self::ConstraintDataType,
            libsqlite3_sys::SQLITE_NOTICE_RECOVER_WAL => Self::NoticeRecoverWal,
            libsqlite3_sys::SQLITE_NOTICE_RECOVER_ROLLBACK => Self::NoticeRecoverRollback,
            libsqlite3_sys::SQLITE_WARNING_AUTOINDEX => Self::WarningAutoIndex,
            libsqlite3_sys::SQLITE_AUTH_USER => Self::AuthUser,
            libsqlite3_sys::SQLITE_OK_LOAD_PERMANENTLY => Self::OkLoadPermanently,
            libsqlite3_sys::SQLITE_OK_SYMLINK => Self::OkSymlink,
            _ => Self::Unknown(code as u32),
        }
    }

    /// Returns `true` when this extended code represents a busy condition.
    pub(crate) fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::BusyRecovery | Self::BusySnapshot | Self::BusyTimeout
        )
    }
}

/// An error returned from Sqlite
#[derive(Debug, Clone, thiserror::Error)]
#[error("(code: {:?}) {message}", .extended)]
pub struct SqliteError {
    /// Primary error code.
    pub primary: PrimaryErrCode,
    /// Extended error code.
    pub extended: ExtendedErrCode,
    /// SQLite-provided error message.
    pub message: String,
}

impl SqliteError {
    /// Build a new error from the active SQLite handle.
    pub(crate) fn new(handle: *mut sqlite3) -> Self {
        let code = ffi::extended_errcode(handle);
        let message = unsafe {
            let msg = ffi::errmsg(handle);
            debug_assert!(!msg.is_null());
            CStr::from_ptr(msg).to_string_lossy().into_owned()
        };

        Self {
            extended: ExtendedErrCode::from_code(code),
            primary: PrimaryErrCode::from_code(code),
            message,
        }
    }

    /// Returns `true` if the error represents a busy condition.
    pub(crate) fn is_busy(&self) -> bool {
        self.primary == PrimaryErrCode::Busy || self.extended.is_busy()
    }

    /// Returns `true` if the error indicates a retryable condition.
    pub(crate) fn should_retry(&self) -> bool {
        self.primary == PrimaryErrCode::Locked
            || self.extended == ExtendedErrCode::LockedSharedCache
            || self.is_busy()
    }
}
