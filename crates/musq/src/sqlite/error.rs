use std::ffi::CStr;

use crate::sqlite::ffi;
use libsqlite3_sys::{self, sqlite3};

// Error Codes And Messages
// https://www.sqlite.org/c3ref/errcode.html

/// Primary Sqlite error codes.
///
/// **Note:** This enum is marked `#[non_exhaustive]`; avoid exhaustive
/// matches as new variants may be introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum PrimaryErrCode {
    Error,
    Internal,
    Perm,
    Abort,
    Busy,
    Locked,
    NoMem,
    ReadOnly,
    Interrupt,
    IoErr,
    Corrupt,
    NotFound,
    Full,
    CantOpen,
    Protocol,
    Empty,
    Schema,
    TooBig,
    Constraint,
    Mismatch,
    Misuse,
    NoLfs,
    Auth,
    Format,
    Range,
    NotADB,
    Notice,
    Warning,
    Unknown(u32),
}

impl PrimaryErrCode {
    fn from_code(code: i32) -> PrimaryErrCode {
        match code & 255 {
            libsqlite3_sys::SQLITE_ERROR => PrimaryErrCode::Error,
            libsqlite3_sys::SQLITE_INTERNAL => PrimaryErrCode::Internal,
            libsqlite3_sys::SQLITE_PERM => PrimaryErrCode::Perm,
            libsqlite3_sys::SQLITE_ABORT => PrimaryErrCode::Abort,
            libsqlite3_sys::SQLITE_BUSY => PrimaryErrCode::Busy,
            libsqlite3_sys::SQLITE_LOCKED => PrimaryErrCode::Locked,
            libsqlite3_sys::SQLITE_NOMEM => PrimaryErrCode::NoMem,
            libsqlite3_sys::SQLITE_READONLY => PrimaryErrCode::ReadOnly,
            libsqlite3_sys::SQLITE_INTERRUPT => PrimaryErrCode::Interrupt,
            libsqlite3_sys::SQLITE_IOERR => PrimaryErrCode::IoErr,
            libsqlite3_sys::SQLITE_CORRUPT => PrimaryErrCode::Corrupt,
            libsqlite3_sys::SQLITE_NOTFOUND => PrimaryErrCode::NotFound,
            libsqlite3_sys::SQLITE_FULL => PrimaryErrCode::Full,
            libsqlite3_sys::SQLITE_CANTOPEN => PrimaryErrCode::CantOpen,
            libsqlite3_sys::SQLITE_PROTOCOL => PrimaryErrCode::Protocol,
            libsqlite3_sys::SQLITE_EMPTY => PrimaryErrCode::Empty,
            libsqlite3_sys::SQLITE_SCHEMA => PrimaryErrCode::Schema,
            libsqlite3_sys::SQLITE_TOOBIG => PrimaryErrCode::TooBig,
            libsqlite3_sys::SQLITE_CONSTRAINT => PrimaryErrCode::Constraint,
            libsqlite3_sys::SQLITE_MISMATCH => PrimaryErrCode::Mismatch,
            libsqlite3_sys::SQLITE_MISUSE => PrimaryErrCode::Misuse,
            libsqlite3_sys::SQLITE_NOLFS => PrimaryErrCode::NoLfs,
            libsqlite3_sys::SQLITE_AUTH => PrimaryErrCode::Auth,
            libsqlite3_sys::SQLITE_FORMAT => PrimaryErrCode::Format,
            libsqlite3_sys::SQLITE_RANGE => PrimaryErrCode::Range,
            libsqlite3_sys::SQLITE_NOTADB => PrimaryErrCode::NotADB,
            libsqlite3_sys::SQLITE_NOTICE => PrimaryErrCode::Notice,
            libsqlite3_sys::SQLITE_WARNING => PrimaryErrCode::Warning,
            _ => PrimaryErrCode::Unknown(code as u32),
        }
    }
}

/// Extended Sqlite error codes.
///
/// **Note:** This enum is marked `#[non_exhaustive]`; avoid exhaustive
/// matches as new variants may be introduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum ExtendedErrCode {
    ErrorMissingCollseq,
    ErrorRetry,
    ErrorSnapshot,
    IOErrRead,
    IOErrShortRead,
    IOErrWrite,
    IOErrFsync,
    IOErrDirFsync,
    IOErrTruncate,
    IOErrFstat,
    IOErrUnlock,
    IOErrRdlock,
    IOErrDelete,
    IOErrBlocked,
    IOErrNoMem,
    IOErrAccess,
    IOErrCheckReservedLock,
    IOErrLock,
    IOErrClose,
    IOErrDirClose,
    IOErrShmopen,
    IOErrShmsize,
    IOErrShmlock,
    IOErrShmmap,
    IOErrSeek,
    IOErrDeleteNoent,
    IOErrMmap,
    IOErrGetTempPath,
    IOErrConvPath,
    IOErrVnode,
    IOErrAuth,
    IOErrBeginAtomic,
    IOErrCommitAtomic,
    IOErrRollbackAtomic,
    IOErrData,
    IOErrCorruptFs,
    LockedSharedCache,
    LockedVTab,
    BusyRecovery,
    BusySnapshot,
    BusyTimeout,
    CantOpenNoTempDir,
    CantOpenIsDir,
    CantOpenFullPath,
    CantOpenConvPath,
    CantOpenDirtyWal,
    CantOpenSymlink,
    CorruptVTab,
    CorruptSequence,
    CorruptIndex,
    ReadOnlyRecovery,
    ReadOnlyCantLock,
    ReadOnlyRollback,
    ReadOnlyDbMoved,
    ReadOnlyCantInit,
    ReadOnlyDirectory,
    AbortRollback,
    ConstraintCheck,
    ConstraintCommitHook,
    ConstraintForeignKey,
    ConstraintFunction,
    ConstraintNotNull,
    ConstraintPrimaryKey,
    ConstraintTrigger,
    ConstraintUnique,
    ConstraintVTab,
    ConstraintRowId,
    ConstraintPinned,
    ConstraintDataType,
    NoticeRecoverWal,
    NoticeRecoverRollback,
    WarningAutoIndex,
    AuthUser,
    OkLoadPermanently,
    OkSymlink,
    Unknown(u32),
}

impl ExtendedErrCode {
    fn from_code(code: i32) -> ExtendedErrCode {
        match code {
            libsqlite3_sys::SQLITE_ERROR_MISSING_COLLSEQ => ExtendedErrCode::ErrorMissingCollseq,
            libsqlite3_sys::SQLITE_ERROR_RETRY => ExtendedErrCode::ErrorRetry,
            libsqlite3_sys::SQLITE_ERROR_SNAPSHOT => ExtendedErrCode::ErrorSnapshot,
            libsqlite3_sys::SQLITE_IOERR_READ => ExtendedErrCode::IOErrRead,
            libsqlite3_sys::SQLITE_IOERR_SHORT_READ => ExtendedErrCode::IOErrShortRead,
            libsqlite3_sys::SQLITE_IOERR_WRITE => ExtendedErrCode::IOErrWrite,
            libsqlite3_sys::SQLITE_IOERR_FSYNC => ExtendedErrCode::IOErrFsync,
            libsqlite3_sys::SQLITE_IOERR_DIR_FSYNC => ExtendedErrCode::IOErrDirFsync,
            libsqlite3_sys::SQLITE_IOERR_TRUNCATE => ExtendedErrCode::IOErrTruncate,
            libsqlite3_sys::SQLITE_IOERR_FSTAT => ExtendedErrCode::IOErrFstat,
            libsqlite3_sys::SQLITE_IOERR_UNLOCK => ExtendedErrCode::IOErrUnlock,
            libsqlite3_sys::SQLITE_IOERR_RDLOCK => ExtendedErrCode::IOErrRdlock,
            libsqlite3_sys::SQLITE_IOERR_DELETE => ExtendedErrCode::IOErrDelete,
            libsqlite3_sys::SQLITE_IOERR_BLOCKED => ExtendedErrCode::IOErrBlocked,
            libsqlite3_sys::SQLITE_IOERR_NOMEM => ExtendedErrCode::IOErrNoMem,
            libsqlite3_sys::SQLITE_IOERR_ACCESS => ExtendedErrCode::IOErrAccess,
            libsqlite3_sys::SQLITE_IOERR_CHECKRESERVEDLOCK => {
                ExtendedErrCode::IOErrCheckReservedLock
            }
            libsqlite3_sys::SQLITE_IOERR_LOCK => ExtendedErrCode::IOErrLock,
            libsqlite3_sys::SQLITE_IOERR_CLOSE => ExtendedErrCode::IOErrClose,
            libsqlite3_sys::SQLITE_IOERR_DIR_CLOSE => ExtendedErrCode::IOErrDirClose,
            libsqlite3_sys::SQLITE_IOERR_SHMOPEN => ExtendedErrCode::IOErrShmopen,
            libsqlite3_sys::SQLITE_IOERR_SHMSIZE => ExtendedErrCode::IOErrShmsize,
            libsqlite3_sys::SQLITE_IOERR_SHMLOCK => ExtendedErrCode::IOErrShmlock,
            libsqlite3_sys::SQLITE_IOERR_SHMMAP => ExtendedErrCode::IOErrShmmap,
            libsqlite3_sys::SQLITE_IOERR_SEEK => ExtendedErrCode::IOErrSeek,
            libsqlite3_sys::SQLITE_IOERR_DELETE_NOENT => ExtendedErrCode::IOErrDeleteNoent,
            libsqlite3_sys::SQLITE_IOERR_MMAP => ExtendedErrCode::IOErrMmap,
            libsqlite3_sys::SQLITE_IOERR_GETTEMPPATH => ExtendedErrCode::IOErrGetTempPath,
            libsqlite3_sys::SQLITE_IOERR_CONVPATH => ExtendedErrCode::IOErrConvPath,
            libsqlite3_sys::SQLITE_IOERR_VNODE => ExtendedErrCode::IOErrVnode,
            libsqlite3_sys::SQLITE_IOERR_AUTH => ExtendedErrCode::IOErrAuth,
            libsqlite3_sys::SQLITE_IOERR_BEGIN_ATOMIC => ExtendedErrCode::IOErrBeginAtomic,
            libsqlite3_sys::SQLITE_IOERR_COMMIT_ATOMIC => ExtendedErrCode::IOErrCommitAtomic,
            libsqlite3_sys::SQLITE_IOERR_ROLLBACK_ATOMIC => ExtendedErrCode::IOErrRollbackAtomic,
            libsqlite3_sys::SQLITE_IOERR_DATA => ExtendedErrCode::IOErrData,
            libsqlite3_sys::SQLITE_IOERR_CORRUPTFS => ExtendedErrCode::IOErrCorruptFs,
            libsqlite3_sys::SQLITE_LOCKED_SHAREDCACHE => ExtendedErrCode::LockedSharedCache,
            libsqlite3_sys::SQLITE_LOCKED_VTAB => ExtendedErrCode::LockedVTab,
            libsqlite3_sys::SQLITE_BUSY_RECOVERY => ExtendedErrCode::BusyRecovery,
            libsqlite3_sys::SQLITE_BUSY_SNAPSHOT => ExtendedErrCode::BusySnapshot,
            libsqlite3_sys::SQLITE_BUSY_TIMEOUT => ExtendedErrCode::BusyTimeout,
            libsqlite3_sys::SQLITE_CANTOPEN_NOTEMPDIR => ExtendedErrCode::CantOpenNoTempDir,
            libsqlite3_sys::SQLITE_CANTOPEN_ISDIR => ExtendedErrCode::CantOpenIsDir,
            libsqlite3_sys::SQLITE_CANTOPEN_FULLPATH => ExtendedErrCode::CantOpenFullPath,
            libsqlite3_sys::SQLITE_CANTOPEN_CONVPATH => ExtendedErrCode::CantOpenConvPath,
            libsqlite3_sys::SQLITE_CANTOPEN_DIRTYWAL => ExtendedErrCode::CantOpenDirtyWal,
            libsqlite3_sys::SQLITE_CANTOPEN_SYMLINK => ExtendedErrCode::CantOpenSymlink,
            libsqlite3_sys::SQLITE_CORRUPT_VTAB => ExtendedErrCode::CorruptVTab,
            libsqlite3_sys::SQLITE_CORRUPT_SEQUENCE => ExtendedErrCode::CorruptSequence,
            libsqlite3_sys::SQLITE_CORRUPT_INDEX => ExtendedErrCode::CorruptIndex,
            libsqlite3_sys::SQLITE_READONLY_RECOVERY => ExtendedErrCode::ReadOnlyRecovery,
            libsqlite3_sys::SQLITE_READONLY_CANTLOCK => ExtendedErrCode::ReadOnlyCantLock,
            libsqlite3_sys::SQLITE_READONLY_ROLLBACK => ExtendedErrCode::ReadOnlyRollback,
            libsqlite3_sys::SQLITE_READONLY_DBMOVED => ExtendedErrCode::ReadOnlyDbMoved,
            libsqlite3_sys::SQLITE_READONLY_CANTINIT => ExtendedErrCode::ReadOnlyCantInit,
            libsqlite3_sys::SQLITE_READONLY_DIRECTORY => ExtendedErrCode::ReadOnlyDirectory,
            libsqlite3_sys::SQLITE_ABORT_ROLLBACK => ExtendedErrCode::AbortRollback,
            libsqlite3_sys::SQLITE_CONSTRAINT_CHECK => ExtendedErrCode::ConstraintCheck,
            libsqlite3_sys::SQLITE_CONSTRAINT_COMMITHOOK => ExtendedErrCode::ConstraintCommitHook,
            libsqlite3_sys::SQLITE_CONSTRAINT_FOREIGNKEY => ExtendedErrCode::ConstraintForeignKey,
            libsqlite3_sys::SQLITE_CONSTRAINT_FUNCTION => ExtendedErrCode::ConstraintFunction,
            libsqlite3_sys::SQLITE_CONSTRAINT_NOTNULL => ExtendedErrCode::ConstraintNotNull,
            libsqlite3_sys::SQLITE_CONSTRAINT_PRIMARYKEY => ExtendedErrCode::ConstraintPrimaryKey,
            libsqlite3_sys::SQLITE_CONSTRAINT_TRIGGER => ExtendedErrCode::ConstraintTrigger,
            libsqlite3_sys::SQLITE_CONSTRAINT_UNIQUE => ExtendedErrCode::ConstraintUnique,
            libsqlite3_sys::SQLITE_CONSTRAINT_VTAB => ExtendedErrCode::ConstraintVTab,
            libsqlite3_sys::SQLITE_CONSTRAINT_ROWID => ExtendedErrCode::ConstraintRowId,
            libsqlite3_sys::SQLITE_CONSTRAINT_PINNED => ExtendedErrCode::ConstraintPinned,
            libsqlite3_sys::SQLITE_CONSTRAINT_DATATYPE => ExtendedErrCode::ConstraintDataType,
            libsqlite3_sys::SQLITE_NOTICE_RECOVER_WAL => ExtendedErrCode::NoticeRecoverWal,
            libsqlite3_sys::SQLITE_NOTICE_RECOVER_ROLLBACK => {
                ExtendedErrCode::NoticeRecoverRollback
            }
            libsqlite3_sys::SQLITE_WARNING_AUTOINDEX => ExtendedErrCode::WarningAutoIndex,
            libsqlite3_sys::SQLITE_AUTH_USER => ExtendedErrCode::AuthUser,
            libsqlite3_sys::SQLITE_OK_LOAD_PERMANENTLY => ExtendedErrCode::OkLoadPermanently,
            libsqlite3_sys::SQLITE_OK_SYMLINK => ExtendedErrCode::OkSymlink,
            _ => ExtendedErrCode::Unknown(code as u32),
        }
    }

    pub(crate) fn is_busy(&self) -> bool {
        matches!(
            self,
            ExtendedErrCode::BusyRecovery
                | ExtendedErrCode::BusySnapshot
                | ExtendedErrCode::BusyTimeout
        )
    }
}

/// An error returned from Sqlite
#[derive(Debug, thiserror::Error)]
#[error("(code: {:?}) {message}", .extended)]
pub struct SqliteError {
    pub(crate) primary: PrimaryErrCode,
    pub(crate) extended: ExtendedErrCode,
    pub message: String,
}

impl SqliteError {
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

    pub(crate) fn is_busy(&self) -> bool {
        self.primary == PrimaryErrCode::Busy || self.extended.is_busy()
    }

    pub(crate) fn should_retry(&self) -> bool {
        self.primary == PrimaryErrCode::Locked
            || self.extended == ExtendedErrCode::LockedSharedCache
            || self.is_busy()
    }

    #[allow(dead_code)]
    pub(crate) fn primary_code(&self) -> PrimaryErrCode {
        self.primary
    }

    #[allow(dead_code)]
    pub(crate) fn extended_code(&self) -> ExtendedErrCode {
        self.extended
    }
}
