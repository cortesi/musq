use libsqlite3_sys::{
    SQLITE_CHECKPOINT_FULL, SQLITE_CHECKPOINT_NOOP, SQLITE_CHECKPOINT_PASSIVE,
    SQLITE_CHECKPOINT_RESTART, SQLITE_CHECKPOINT_TRUNCATE, SQLITE_DBSTATUS_CACHE_HIT,
    SQLITE_DBSTATUS_CACHE_MISS, SQLITE_DBSTATUS_CACHE_SPILL, SQLITE_DBSTATUS_CACHE_USED,
    SQLITE_DBSTATUS_CACHE_USED_SHARED, SQLITE_DBSTATUS_CACHE_WRITE, SQLITE_DBSTATUS_DEFERRED_FKS,
    SQLITE_DBSTATUS_LOOKASIDE_HIT, SQLITE_DBSTATUS_LOOKASIDE_MISS_FULL,
    SQLITE_DBSTATUS_LOOKASIDE_MISS_SIZE, SQLITE_DBSTATUS_LOOKASIDE_USED,
    SQLITE_DBSTATUS_SCHEMA_USED, SQLITE_DBSTATUS_STMT_USED, SQLITE_DBSTATUS_TEMPBUF_SPILL,
};

/// Runtime identity and compile options for the bundled SQLite library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteRuntimeInfo {
    /// SQLite semantic version string, such as `3.53.2`.
    pub version: String,
    /// SQLite numeric version, such as `3053002` for `3.53.2`.
    pub version_number: i32,
    /// SQLite source identifier string for the active runtime.
    pub source_id: String,
    /// Compile options reported by the active SQLite runtime.
    pub compile_options: Vec<String>,
}

/// A database-connection status measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DbStatus {
    /// Current value for the requested status counter.
    pub current: i64,
    /// High-water value for the requested status counter.
    pub highwater: i64,
}

/// Per-connection SQLite status counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DbStatusKind {
    /// Number of lookaside memory slots currently checked out.
    LookasideUsed,
    /// Bytes of page cache memory currently used by this connection.
    CacheUsed,
    /// Bytes of schema memory currently used by this connection.
    SchemaUsed,
    /// Bytes of memory currently used by prepared statements on this connection.
    StatementUsed,
    /// Lookaside allocation hit count.
    LookasideHit,
    /// Lookaside allocation miss count because the allocation was too large.
    LookasideMissSize,
    /// Lookaside allocation miss count because all slots were already used.
    LookasideMissFull,
    /// Page cache hit count.
    CacheHit,
    /// Page cache miss count.
    CacheMiss,
    /// Page cache write count.
    CacheWrite,
    /// Number of deferred foreign key constraints that have not yet been resolved.
    DeferredForeignKeys,
    /// Bytes of shared page cache memory attributed to this connection.
    CacheUsedShared,
    /// Dirty page spill count.
    CacheSpill,
    /// Temporary buffer spill count.
    TempBufferSpill,
}

impl DbStatusKind {
    /// Return the SQLite status opcode for this kind.
    pub(crate) fn as_sqlite_code(self) -> i32 {
        match self {
            Self::LookasideUsed => SQLITE_DBSTATUS_LOOKASIDE_USED,
            Self::CacheUsed => SQLITE_DBSTATUS_CACHE_USED,
            Self::SchemaUsed => SQLITE_DBSTATUS_SCHEMA_USED,
            Self::StatementUsed => SQLITE_DBSTATUS_STMT_USED,
            Self::LookasideHit => SQLITE_DBSTATUS_LOOKASIDE_HIT,
            Self::LookasideMissSize => SQLITE_DBSTATUS_LOOKASIDE_MISS_SIZE,
            Self::LookasideMissFull => SQLITE_DBSTATUS_LOOKASIDE_MISS_FULL,
            Self::CacheHit => SQLITE_DBSTATUS_CACHE_HIT,
            Self::CacheMiss => SQLITE_DBSTATUS_CACHE_MISS,
            Self::CacheWrite => SQLITE_DBSTATUS_CACHE_WRITE,
            Self::DeferredForeignKeys => SQLITE_DBSTATUS_DEFERRED_FKS,
            Self::CacheUsedShared => SQLITE_DBSTATUS_CACHE_USED_SHARED,
            Self::CacheSpill => SQLITE_DBSTATUS_CACHE_SPILL,
            Self::TempBufferSpill => SQLITE_DBSTATUS_TEMPBUF_SPILL,
        }
    }
}

/// WAL checkpoint mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum WalCheckpointMode {
    /// Return WAL status without checkpointing frames.
    Noop,
    /// Checkpoint without waiting for readers or writers.
    Passive,
    /// Wait for writers, then checkpoint all frames possible without blocking readers.
    Full,
    /// Like full checkpointing, and reset the WAL if all frames are checkpointed.
    Restart,
    /// Like restart checkpointing, and truncate the WAL to zero bytes on success.
    Truncate,
}

impl WalCheckpointMode {
    /// Return the SQLite checkpoint opcode for this mode.
    pub(crate) fn as_sqlite_code(self) -> i32 {
        match self {
            Self::Noop => SQLITE_CHECKPOINT_NOOP,
            Self::Passive => SQLITE_CHECKPOINT_PASSIVE,
            Self::Full => SQLITE_CHECKPOINT_FULL,
            Self::Restart => SQLITE_CHECKPOINT_RESTART,
            Self::Truncate => SQLITE_CHECKPOINT_TRUNCATE,
        }
    }
}

/// Result of a WAL checkpoint operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalCheckpoint {
    /// Total frames in the WAL, or `None` when SQLite reports `-1`.
    pub log_frames: Option<i32>,
    /// Frames checkpointed from the WAL, or `None` when SQLite reports `-1`.
    pub checkpointed_frames: Option<i32>,
}
