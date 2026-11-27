//! Arc-backed SQLite database host with canonical WAL bootstrap, lazy
//! single-threaded FIFO writer, and standalone archive export.

use std::{
    env,
    ffi::OsString,
    fs, io,
    ops::AsyncFnOnce,
    path::{Path, PathBuf},
    result,
    sync::{Arc, PoisonError, RwLock},
    time::Duration,
};

use indexmap::IndexMap;

#[cfg(feature = "json")]
pub use self::sandbox::{json_params_to_values, value_to_json};
use self::writer::{Commit, LazyWriter};
pub use self::{
    module::{Module, ModuleRecord},
    registry::DatabaseRegistry,
    sandbox::{
        ReadOnlySandbox, SandboxQueryOptions, SandboxQueryResult, fleet_query,
        validate_single_statement,
    },
    surface::{QuerySurface, SandboxPrepareHook},
};
use crate::{
    AutoVacuum, Connection, Error, JournalMode, Musq, Pool, Result, Synchronous, Transaction,
    TransactionBehavior, query, query_scalar,
};

/// Composable schema modules and `_musq_modules` registry.
mod module;
/// Canonical-path registry of shared writable database handles.
mod registry;
/// Composable read-only SQL sandbox and multi-file fleet queries.
mod sandbox;
/// Declarative read-only SQL query surfaces.
mod surface;
/// Dedicated single-threaded FIFO writer runtime.
mod writer;

/// Default SQLite busy timeout (60 seconds).
pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(60);
/// Default SQLite per-connection page cache size (`-8192` = 8 MiB).
pub const DEFAULT_CACHE_SIZE_KIB: i32 = -8192;
/// Default maximum pooled connections for [`Database`].
pub const DEFAULT_MAX_CONNECTIONS: u32 = 10;
/// Default pool acquire timeout for [`Database`].
pub const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
/// Default thread name for the lazy writer thread.
pub const DEFAULT_WRITER_THREAD_NAME: &str = "musq-writer";

/// DDL creating the physical module registry table on writable host startup.
const CREATE_MODULES_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS _musq_modules (
    module_id TEXT PRIMARY KEY,
    table_prefix TEXT NOT NULL UNIQUE,
    schema_version INTEGER NOT NULL,
    installed_at TEXT NOT NULL,
    meta_json TEXT NOT NULL DEFAULT '{}'
)";

/// Configuration for opening a [`Database`] host.
#[derive(Debug, Clone)]
pub struct DatabaseOptions {
    /// SQLite busy timeout (default: 60 seconds).
    pub busy_timeout: Duration,
    /// SQLite `PRAGMA cache_size` value (negative = KiB; default: `-8192`).
    pub cache_size_kib: i32,
    /// SQLite `PRAGMA mmap_size` in bytes (default: `0`).
    pub mmap_size_bytes: u64,
    /// Maximum number of pooled connections for concurrent reads.
    pub max_connections: u32,
    /// Timeout when acquiring a connection from the pool.
    pub acquire_timeout: Duration,
    /// Optional host-owned `PRAGMA application_id`.
    ///
    /// On a fresh writable database, the host writes this value. On an existing
    /// database, the host requires the stored `application_id` to match.
    pub application_id: Option<i32>,
    /// Optional host-owned `PRAGMA user_version`.
    ///
    /// On a fresh writable database, the host writes this value. On an existing
    /// database, the host requires the stored `user_version` to match unless
    /// [`Self::stamp_header_if_fresh`] is `true`.
    pub user_version: Option<i32>,
    /// When `true`, stamp `application_id` and `user_version` onto fresh
    /// databases without rejecting existing databases that carry another host's
    /// header values.
    pub stamp_header_if_fresh: bool,
    /// Thread name for the lazy single-threaded FIFO writer runtime.
    pub writer_thread_name: String,
}

impl Default for DatabaseOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseOptions {
    /// Construct default [`DatabaseOptions`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
            cache_size_kib: DEFAULT_CACHE_SIZE_KIB,
            mmap_size_bytes: 0,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            acquire_timeout: DEFAULT_ACQUIRE_TIMEOUT,
            application_id: None,
            user_version: None,
            stamp_header_if_fresh: false,
            writer_thread_name: DEFAULT_WRITER_THREAD_NAME.to_owned(),
        }
    }

    /// Set the SQLite busy timeout.
    #[must_use]
    pub const fn busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = timeout;
        self
    }

    /// Set `PRAGMA cache_size` (negative values specify KiB).
    #[must_use]
    pub const fn cache_size_kib(mut self, cache_size_kib: i32) -> Self {
        self.cache_size_kib = cache_size_kib;
        self
    }

    /// Set `PRAGMA mmap_size` in bytes.
    #[must_use]
    pub const fn mmap_size_bytes(mut self, mmap_size_bytes: u64) -> Self {
        self.mmap_size_bytes = mmap_size_bytes;
        self
    }

    /// Set the maximum number of pooled read connections.
    #[must_use]
    pub const fn max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections;
        self
    }

    /// Set the pool acquire timeout.
    #[must_use]
    pub const fn acquire_timeout(mut self, acquire_timeout: Duration) -> Self {
        self.acquire_timeout = acquire_timeout;
        self
    }

    /// Set the host-owned `PRAGMA application_id`.
    #[must_use]
    pub const fn application_id(mut self, application_id: i32) -> Self {
        self.application_id = Some(application_id);
        self
    }

    /// Set the host-owned `PRAGMA user_version`.
    #[must_use]
    pub const fn user_version(mut self, user_version: i32) -> Self {
        self.user_version = Some(user_version);
        self
    }

    /// Stamp `application_id` and `user_version` only on fresh databases,
    /// accepting existing databases with any header values.
    #[must_use]
    pub const fn stamp_header_if_fresh(mut self) -> Self {
        self.stamp_header_if_fresh = true;
        self
    }

    /// Set the thread name for the lazy writer thread.
    #[must_use]
    pub fn writer_thread_name(mut self, name: impl Into<String>) -> Self {
        self.writer_thread_name = name.into();
        self
    }
}

/// Shared inner state for a [`Database`] host.
#[derive(Debug)]
struct DatabaseInner {
    /// Canonical filesystem path of the database file.
    path: PathBuf,
    /// Shared connection pool for concurrent reads.
    pool: Pool,
    /// Lazy single-threaded FIFO writer (`None` for read-only handles).
    writer: Option<LazyWriter>,
    /// Query surfaces contributed by mounted or explicitly registered modules,
    /// keyed by `module_id` in registration order.
    surfaces: RwLock<IndexMap<String, QuerySurface>>,
    /// Whether this handle was opened read-only.
    read_only: bool,
    /// Whether this handle was opened with SQLite `immutable=1`.
    immutable: bool,
}

/// Arc-backed, cloneable SQLite database host.
///
/// `Database` pairs a shared [`Pool`] for concurrent reads with a lazy
/// single-threaded FIFO writer that serializes every write transaction onto one
/// dedicated SQLite connection.
#[derive(Debug, Clone)]
pub struct Database(Arc<DatabaseInner>);

impl Database {
    /// Open or create a writable database at `path` with canonical WAL
    /// bootstrap and `_musq_modules` registry initialization.
    pub async fn open(path: impl AsRef<Path>, options: DatabaseOptions) -> Result<Self> {
        let path = path.as_ref();
        ensure_parent_dir(path)?;
        bootstrap_new_wal_file(path, options.busy_timeout).await?;

        let musq = build_writable_musq(&options);
        let pool = musq.clone().open(path).await?;
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        if let Err(err) = init_writable_database(&pool, &canonical, &options).await {
            let () = pool.close().await;
            return Err(err);
        }

        let writer_opts = build_writable_musq(&options).filename(&canonical);
        let writer = LazyWriter::new(writer_opts, options.writer_thread_name);

        Ok(Self(Arc::new(DatabaseInner {
            path: canonical,
            pool,
            writer: Some(writer),
            surfaces: RwLock::new(IndexMap::new()),
            read_only: false,
            immutable: false,
        })))
    }

    /// Open an existing database file read-only using default options.
    ///
    /// Does not enforce `application_id` or `user_version`, allowing read-only
    /// consumers to inspect any compatible SQLite database file.
    pub async fn open_read_only(path: impl AsRef<Path>, immutable: bool) -> Result<Self> {
        Self::open_read_only_with_options(path, immutable, DatabaseOptions::new()).await
    }

    /// Open an existing database file read-only with explicit
    /// [`DatabaseOptions`].
    ///
    /// An `immutable` open reads the main file alone, so it refuses a file
    /// whose write-ahead log holds frames, such as a database that a writer
    /// holds open: such a file is not an archive.
    pub async fn open_read_only_with_options(
        path: impl AsRef<Path>,
        immutable: bool,
        options: DatabaseOptions,
    ) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::NotFound,
                format!("database file `{}` does not exist", path.display()),
            )));
        }
        if immutable {
            refuse_unsettled_wal(path)?;
        }

        let musq = Musq::new()
            .read_only(true)
            .immutable(immutable)
            .busy_timeout(options.busy_timeout)
            .max_connections(options.max_connections)
            .acquire_timeout(options.acquire_timeout)
            .pragma("temp_store", "FILE")
            .pragma("cache_size", &options.cache_size_kib.to_string())
            .pragma("mmap_size", &options.mmap_size_bytes.to_string());

        let pool = musq.open(path).await?;
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        if let Err(err) = verify_read_only_header(&pool, &canonical, &options).await {
            let () = pool.close().await;
            return Err(err);
        }

        Ok(Self(Arc::new(DatabaseInner {
            path: canonical,
            pool,
            writer: None,
            surfaces: RwLock::new(IndexMap::new()),
            read_only: true,
            immutable,
        })))
    }

    /// Return the shared read [`Pool`].
    #[must_use]
    pub fn pool(&self) -> &Pool {
        &self.0.pool
    }

    /// Return the canonical filesystem path of this database.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0.path
    }

    /// Return `true` if this host was opened read-only.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.0.read_only
    }

    /// Return `true` if this host was opened with SQLite `immutable=1`.
    #[must_use]
    pub fn is_immutable(&self) -> bool {
        self.0.immutable
    }

    /// Execute `callback` inside an atomic `BEGIN IMMEDIATE` transaction on the
    /// lazy single-threaded FIFO writer connection.
    ///
    /// Commits when `callback` returns `Ok`, and rolls back cleanly when
    /// `callback` returns `Err` or panics. Returns [`Error::ReadOnly`] on
    /// read-only hosts.
    pub async fn write<F, R, E>(&self, callback: F) -> result::Result<R, E>
    where
        F: AsyncFnOnce(&mut Transaction<&mut Connection>) -> result::Result<R, E> + Send + 'static,
        R: Send + 'static,
        E: From<Error> + Send + 'static,
    {
        let Some(writer) = &self.0.writer else {
            return Err(E::from(Error::ReadOnly));
        };
        writer.run_tx(Commit::Settled, callback).await
    }

    /// Execute `callback` as [`Self::write`] does, with a durable commit: the
    /// commit syncs the write-ahead log to the disk, so that this transaction
    /// and each transaction committed before it survive a power loss.
    ///
    /// In WAL mode with `synchronous = NORMAL`, the setting of a writable
    /// host, a commit reaches the disk only at a later checkpoint. This write
    /// runs with `synchronous = FULL`, and the writer then takes its setting
    /// back. Unlike a `FULL` checkpoint, it waits for no reader and copies no
    /// page to the database file.
    ///
    /// `callback` must change at least one page, such as by an `UPDATE` that
    /// matches a row. A commit that changes no page appends no frame to the
    /// write-ahead log, so it syncs nothing, and the earlier commits stay as
    /// durable as they were.
    pub async fn write_durable<F, R, E>(&self, callback: F) -> result::Result<R, E>
    where
        F: AsyncFnOnce(&mut Transaction<&mut Connection>) -> result::Result<R, E> + Send + 'static,
        R: Send + 'static,
        E: From<Error> + Send + 'static,
    {
        let Some(writer) = &self.0.writer else {
            return Err(E::from(Error::ReadOnly));
        };
        writer.run_tx(Commit::Durable, callback).await
    }

    /// Mount a [`Module`] onto this writable database host.
    ///
    /// Enforces `DEPENDS_ON` ordering, unique `TABLE_PREFIX` ownership,
    /// `sqlite_schema` prefix and target-table checks, `PRAGMA
    /// defer_foreign_keys = ON` + `PRAGMA foreign_key_check` on upgrades,
    /// idempotency at matching schema version, and downgrade rejection. On
    /// success, records `module.query_surface()` on this host handle.
    ///
    /// A done mount, with `M` in `_musq_modules` at [`Module::SCHEMA_VERSION`],
    /// reads the registry through the read pool and queues no writer job. Each
    /// other mount queues a writer job, which decides again: an install, an
    /// upgrade, a refusal (such as a downgrade, a prefix mismatch, or a
    /// missing dependency), and a mount whose read of the registry failed.
    pub async fn mount<M: Module>(&self, module: M) -> Result<ModuleRecord> {
        if self.is_read_only() {
            return Err(Error::ReadOnly);
        }
        module::validate_module_identity(M::ID, M::TABLE_PREFIX)?;

        let surface = module.query_surface();
        let record = match module::done_mount_from_pool::<M>(&self.0.pool).await {
            Some(record) => record,
            None => {
                let meta_json = module.meta_json();
                self.write(async move |tx| module::mount_in_tx::<M>(tx, &meta_json).await)
                    .await?
            }
        };

        self.register_query_surface(M::ID, surface);
        Ok(record)
    }

    /// List all modules recorded in `_musq_modules`, ordered by `module_id`.
    ///
    /// Returns an empty vector if `_musq_modules` does not exist.
    pub async fn mounted_modules(&self) -> Result<Vec<ModuleRecord>> {
        module::list_modules_from_pool(&self.0.pool).await
    }

    /// Look up one module record by `module_id` in `_musq_modules`.
    pub async fn module_record(&self, module_id: &str) -> Result<Option<ModuleRecord>> {
        module::load_module_from_pool(&self.0.pool, module_id).await
    }

    /// Verify that module `M` is installed in `_musq_modules` with matching
    /// `TABLE_PREFIX` and an installed `schema_version` in
    /// `M::MIN_READ_VERSION..=M::SCHEMA_VERSION`.
    ///
    /// Works on both writable and read-only hosts regardless of host
    /// `application_id` or `user_version`.
    pub async fn verify_module<M: Module>(&self) -> Result<ModuleRecord> {
        module::verify_module_from_pool::<M>(&self.0.pool).await
    }

    /// Register or replace a [`QuerySurface`] associated with `module_id` on
    /// this host handle (for example after read-only module discovery).
    pub fn register_query_surface(&self, module_id: impl Into<String>, surface: QuerySurface) {
        let mut guard = self
            .0
            .surfaces
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        guard.insert(module_id.into(), surface);
    }

    /// Return the combined [`QuerySurface`] union of every mounted or
    /// registered module on this host handle.
    #[must_use]
    pub fn query_surface(&self) -> QuerySurface {
        let guard = self
            .0
            .surfaces
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        QuerySurface::union(guard.values())
    }

    /// Export a standalone, compacted SQLite archive to `dest_path`.
    ///
    /// Runs `VACUUM INTO`, switches the exported file to
    /// `PRAGMA journal_mode = DELETE`, deletes any `-wal` or `-shm` sidecars,
    /// and verifies that the archive opens with `immutable = true` and passes
    /// `PRAGMA quick_check`.
    ///
    /// Returns [`Error::Configuration`] if `dest_path` aliases this database
    /// file or its WAL/SHM sidecars.
    pub async fn export_archive(&self, dest_path: impl AsRef<Path>) -> Result<()> {
        let dest = dest_path.as_ref();
        ensure_distinct_export_path(&self.0.path, dest)?;
        ensure_parent_dir(dest)?;
        remove_if_exists(dest)?;
        remove_sidecars(dest)?;

        self.0.pool.vacuum_into(dest).await?;

        let archive_pool = Musq::new().max_connections(1).open(dest).await?;
        let journal_res: Result<String> = query_scalar("PRAGMA journal_mode = DELETE")
            .fetch_one(&archive_pool)
            .await;
        let () = archive_pool.close().await;
        let journal_mode = journal_res?;
        if !journal_mode.eq_ignore_ascii_case("delete") {
            return Err(Error::ArchiveVerification(format!(
                "expected archive journal_mode `delete`, got `{journal_mode}`"
            )));
        }

        remove_sidecars(dest)?;

        let ro_pool = Musq::new()
            .read_only(true)
            .immutable(true)
            .max_connections(1)
            .open(dest)
            .await?;
        let check_res = ro_pool.quick_check().await;
        let () = ro_pool.close().await;
        let report = check_res?;
        if !report.is_ok() {
            return Err(Error::ArchiveVerification(format!(
                "exported archive failed quick_check: {}",
                report.messages.join("; ")
            )));
        }

        Ok(())
    }

    /// Close the dedicated writer (if started) and shut down the shared
    /// connection pool.
    pub async fn close(&self) -> Result<()> {
        let writer_res = match &self.0.writer {
            Some(writer) => writer.close().await,
            None => Ok(()),
        };
        let () = self.0.pool.close().await;
        writer_res
    }
}

/// Build the standard writable [`Musq`] configuration from [`DatabaseOptions`].
fn build_writable_musq(options: &DatabaseOptions) -> Musq {
    Musq::new()
        .create_if_missing(true)
        .busy_timeout(options.busy_timeout)
        .foreign_keys(true)
        .journal_mode(JournalMode::Wal)
        .synchronous(Synchronous::Normal)
        .max_connections(options.max_connections)
        .acquire_timeout(options.acquire_timeout)
        .pragma("temp_store", "FILE")
        .pragma("cache_size", &options.cache_size_kib.to_string())
        .pragma("mmap_size", &options.mmap_size_bytes.to_string())
}

/// Reject `dest` when it resolves to the same file as `source` or its
/// `-wal`/`-shm` sidecars.
fn ensure_distinct_export_path(source: &Path, dest: &Path) -> Result<()> {
    let resolved_dest = resolve_candidate_path(dest);
    let protected = [
        resolve_candidate_path(source),
        resolve_candidate_path(&sidecar_path(source, "-wal")),
        resolve_candidate_path(&sidecar_path(source, "-shm")),
    ];
    if protected.iter().any(|path| path == &resolved_dest) {
        return Err(Error::Configuration(format!(
            "archive destination `{}` must not alias source database `{}`",
            dest.display(),
            source.display()
        )));
    }
    Ok(())
}

/// Resolve `path` to a canonical form even when the final file does not yet
/// exist on disk.
fn resolve_candidate_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    if let (Some(parent), Some(file_name)) = (path.parent(), path.file_name()) {
        let parent_dir = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        if let Ok(canonical_parent) = fs::canonicalize(parent_dir) {
            return canonical_parent.join(file_name);
        }
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

/// Create parent directories for `path` when needed.
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Apply `page_size = 4096`, `auto_vacuum = INCREMENTAL`, and
/// `journal_mode = WAL` to a brand-new database file before its first page is
/// written.
async fn bootstrap_new_wal_file(path: &Path, busy_timeout: Duration) -> Result<()> {
    let existed = path.exists() && fs::metadata(path).is_ok_and(|meta| meta.len() > 0);
    if existed {
        return Ok(());
    }

    let bootstrap = Musq::new()
        .create_if_missing(true)
        .busy_timeout(busy_timeout)
        .page_size(4096)
        .auto_vacuum(AutoVacuum::Incremental)
        .max_connections(1)
        .open(path)
        .await?;
    let wal_res = query("PRAGMA journal_mode = WAL").execute(&bootstrap).await;
    let () = bootstrap.close().await;
    wal_res?;
    Ok(())
}

/// Return `true` when the read pool already shows an initialized database
/// (with `_musq_modules` present and header fields compatible with `options`),
/// avoiding a `BEGIN IMMEDIATE` write lock behind an active writer.
///
/// Return `false` whenever initialization or authoritative validation must run
/// inside `BEGIN IMMEDIATE`: a fresh database, missing `_musq_modules`, a
/// header mismatch, or a read failure.
async fn writable_database_ready(pool: &Pool, path: &Path, options: &DatabaseOptions) -> bool {
    let Ok(mut conn) = pool.acquire().await else {
        return false;
    };
    let Ok(tx) = conn.begin_with(TransactionBehavior::Deferred).await else {
        return false;
    };
    let ready = async {
        let has_modules: i64 = query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = '_musq_modules'",
        )
        .fetch_one(&*tx)
        .await?;
        if has_modules == 0 {
            return Ok(false);
        }
        if options.stamp_header_if_fresh
            || (options.application_id.is_none() && options.user_version.is_none())
        {
            return Ok(true);
        }
        let app_id: i32 = query_scalar("PRAGMA application_id")
            .fetch_one(&*tx)
            .await?;
        let user_ver: i32 = query_scalar("PRAGMA user_version").fetch_one(&*tx).await?;
        Ok::<bool, Error>(check_header_match(path, app_id, user_ver, options).is_ok())
    }
    .await;
    tx.rollback().await.ok();
    ready.unwrap_or(false)
}

/// Verify or stamp host header fields and ensure `_musq_modules` exists.
async fn init_writable_database(pool: &Pool, path: &Path, options: &DatabaseOptions) -> Result<()> {
    if writable_database_ready(pool, path, options).await {
        return Ok(());
    }
    let mut conn = pool.acquire().await?;
    let tx = conn.begin_with(TransactionBehavior::Immediate).await?;

    let app_id: i32 = query_scalar("PRAGMA application_id")
        .fetch_one(&*tx)
        .await?;
    let user_ver: i32 = query_scalar("PRAGMA user_version").fetch_one(&*tx).await?;
    let table_count: i64 = query_scalar(
        "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_one(&*tx)
    .await?;

    let is_fresh = app_id == 0 && user_ver == 0 && table_count == 0;
    if is_fresh {
        if let Some(expected_app) = options.application_id {
            query(&format!("PRAGMA application_id = {expected_app}"))
                .execute(&*tx)
                .await?;
        }
        if let Some(expected_ver) = options.user_version {
            query(&format!("PRAGMA user_version = {expected_ver}"))
                .execute(&*tx)
                .await?;
        }
    } else if !options.stamp_header_if_fresh
        && let Err(err) = check_header_match(path, app_id, user_ver, options)
    {
        tx.rollback().await.ok();
        return Err(err);
    }

    query(CREATE_MODULES_TABLE_SQL).execute(&*tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Verify configured host header fields on a read-only connection.
async fn verify_read_only_header(
    pool: &Pool,
    path: &Path,
    options: &DatabaseOptions,
) -> Result<()> {
    if options.stamp_header_if_fresh
        || (options.application_id.is_none() && options.user_version.is_none())
    {
        return Ok(());
    }
    let app_id: i32 = query_scalar("PRAGMA application_id")
        .fetch_one(pool)
        .await?;
    let user_ver: i32 = query_scalar("PRAGMA user_version").fetch_one(pool).await?;
    check_header_match(path, app_id, user_ver, options)
}

/// Validate stored `application_id` and `user_version` against configured
/// expectations.
fn check_header_match(
    path: &Path,
    app_id: i32,
    user_ver: i32,
    options: &DatabaseOptions,
) -> Result<()> {
    if let Some(expected_app) = options.application_id
        && app_id != expected_app
    {
        return Err(Error::IncompatibleFormat {
            path: path.to_path_buf(),
            reason: format!("expected application_id={expected_app:#x}, got {app_id:#x}"),
        });
    }
    if let Some(expected_ver) = options.user_version
        && user_ver != expected_ver
    {
        return Err(Error::IncompatibleFormat {
            path: path.to_path_buf(),
            reason: format!("expected user_version={expected_ver}, got {user_ver}"),
        });
    }
    Ok(())
}

/// Remove a file if it exists on disk.
fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Remove SQLite `-wal` and `-shm` sidecar files beside `path`.
fn remove_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm"] {
        remove_if_exists(&sidecar_path(path, suffix))?;
    }
    Ok(())
}

/// Refuses an immutable open of `path` while its write-ahead log holds
/// frames: SQLite reads an immutable database from its main file alone, so
/// it would miss each commit that no checkpoint has copied there.
fn refuse_unsettled_wal(path: &Path) -> Result<()> {
    let wal = sidecar_path(path, "-wal");
    match fs::metadata(&wal) {
        Ok(meta) if meta.len() > 0 => Err(Error::Configuration(format!(
            "database file `{}` has a write-ahead log with frames, which an immutable open \
             does not read: open it without `immutable`, or export or checkpoint it first",
            path.display()
        ))),
        _ => Ok(()),
    }
}

/// Compute a SQLite sidecar path without requiring UTF-8 filenames.
fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut os = OsString::from(path.as_os_str());
    os.push(suffix);
    PathBuf::from(os)
}
