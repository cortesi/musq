//! Canonical-path registry of shared writable [`Database`] handles.

#[cfg(not(unix))]
use std::time::SystemTime;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::Mutex;

use crate::{Database, DatabaseOptions, Result};

/// On-disk identity of a database file so deleted-and-recreated paths do not
/// reuse a stale [`Database`] handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    /// Device ID containing the file.
    #[cfg(unix)]
    dev: u64,
    /// Inode number of the file.
    #[cfg(unix)]
    ino: u64,
    /// File length in bytes.
    #[cfg(not(unix))]
    len: u64,
    /// File modification timestamp, when available.
    #[cfg(not(unix))]
    modified: Option<SystemTime>,
}

/// Captures the on-disk identity of `path`, if it exists.
fn file_identity(path: &Path) -> Option<FileIdentity> {
    let meta = fs::metadata(path).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        Some(FileIdentity {
            dev: meta.dev(),
            ino: meta.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        Some(FileIdentity {
            len: meta.len(),
            modified: meta.modified().ok(),
        })
    }
}

/// Cached writable [`Database`] entry in [`DatabaseRegistry`].
#[derive(Debug)]
struct RegisteredDb {
    /// Underlying shared database handle (single writer + shared read pool).
    db: Database,
    /// On-disk file identity recorded when the database was opened.
    identity: Option<FileIdentity>,
}

/// Host-owned canonical-path registry of shared writable [`Database`] handles.
///
/// Multiple providers or subsystems opening the same canonical path receive
/// clones of one [`Database`] instance (sharing one read pool and one
/// serialized FIFO writer). If the file on disk is deleted or replaced, or if
/// the cached pool is closed, the stale entry is evicted and a fresh handle is
/// opened.
#[derive(Clone, Debug, Default)]
pub struct DatabaseRegistry {
    /// Open writable databases keyed by canonical filesystem path.
    databases: Arc<Mutex<HashMap<PathBuf, RegisteredDb>>>,
}

impl DatabaseRegistry {
    /// Construct an empty [`DatabaseRegistry`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a shared writable [`Database`] for `canonical`, opening it with
    /// `options` on first use or reopening it if the file was deleted/replaced
    /// or its pool was closed.
    pub async fn open_writable(
        &self,
        canonical: &Path,
        options: DatabaseOptions,
    ) -> Result<Database> {
        let mut guard = self.databases.lock().await;
        if let Some(entry) = guard.get(canonical) {
            let current_id = file_identity(canonical);
            let same_file = entry.identity.is_some() && entry.identity == current_id;
            if same_file && !entry.db.pool().is_closed() {
                return Ok(entry.db.clone());
            }
            guard.remove(canonical);
        }

        let db = Database::open(canonical, options).await?;
        let identity = file_identity(canonical);
        guard.insert(
            canonical.to_path_buf(),
            RegisteredDb {
                db: db.clone(),
                identity,
            },
        );
        Ok(db)
    }

    /// Remove and return any cached [`Database`] registered for `canonical`.
    pub async fn remove(&self, canonical: &Path) -> Option<Database> {
        self.databases
            .lock()
            .await
            .remove(canonical)
            .map(|entry| entry.db)
    }

    /// Return the number of currently cached writable database entries.
    pub async fn len(&self) -> usize {
        self.databases.lock().await.len()
    }

    /// Return `true` when no writable database entries are cached.
    pub async fn is_empty(&self) -> bool {
        self.databases.lock().await.is_empty()
    }
}
