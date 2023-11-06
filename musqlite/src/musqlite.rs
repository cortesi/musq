use std::{
    fmt::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
    time::Duration,
};

use crate::{
    debugfn::DebugFn, executor::Executor, logger::LogSettings, pool, pool::PoolOptions,
    sqlite::Connection, Result,
};

use log::LevelFilter;

use indexmap::IndexMap;

static IN_MEMORY_DB_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Refer to [SQLite documentation] for the meaning of the connection locking mode.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_locking_mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LockingMode {
    #[default]
    Normal,
    Exclusive,
}

impl LockingMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            LockingMode::Normal => "NORMAL",
            LockingMode::Exclusive => "EXCLUSIVE",
        }
    }
}

/// Refer to [SQLite documentation] for the meaning of the database journaling mode.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_journal_mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum JournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    #[default]
    Wal,
    Off,
}

impl JournalMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            JournalMode::Delete => "DELETE",
            JournalMode::Truncate => "TRUNCATE",
            JournalMode::Persist => "PERSIST",
            JournalMode::Memory => "MEMORY",
            JournalMode::Wal => "WAL",
            JournalMode::Off => "OFF",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AutoVacuum {
    #[default]
    None,
    Full,
    Incremental,
}

impl AutoVacuum {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            AutoVacuum::None => "NONE",
            AutoVacuum::Full => "FULL",
            AutoVacuum::Incremental => "INCREMENTAL",
        }
    }
}

/// Refer to [SQLite documentation] for the meaning of various synchronous settings.
///
/// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_synchronous
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Synchronous {
    Off,
    Normal,
    #[default]
    Full,
    Extra,
}

impl Synchronous {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Synchronous::Off => "OFF",
            Synchronous::Normal => "NORMAL",
            Synchronous::Full => "FULL",
            Synchronous::Extra => "EXTRA",
        }
    }
}

/// Create a muSQLite connection
#[derive(Clone, Debug)]
pub struct MuSQLite {
    pub(crate) filename: PathBuf,
    pub(crate) in_memory: bool,
    pub(crate) read_only: bool,
    pub(crate) create_if_missing: bool,
    pub(crate) shared_cache: bool,
    pub(crate) statement_cache_capacity: usize,
    pub(crate) busy_timeout: Duration,
    pub(crate) log_settings: LogSettings,
    pub(crate) immutable: bool,
    pub(crate) vfs: Option<String>,

    pub(crate) pragmas: IndexMap<String, Option<String>>,

    pub(crate) command_channel_size: usize,
    pub(crate) row_channel_size: usize,

    pub(crate) serialized: bool,
    pub(crate) thread_name: Arc<DebugFn<dyn Fn(u64) -> String + Send + Sync + 'static>>,

    pub(crate) optimize_on_close: OptimizeOnClose,
}

#[derive(Clone, Debug)]
pub enum OptimizeOnClose {
    Enabled { analysis_limit: Option<u32> },
    Disabled,
}

impl Default for MuSQLite {
    fn default() -> Self {
        Self::new()
    }
}

impl MuSQLite {
    /// Construct `Self` with default options.
    ///
    /// See the source of this method for the current defaults.
    pub fn new() -> Self {
        let mut pragmas: IndexMap<String, Option<String>> = IndexMap::new();

        // Standard pragmas
        //
        // Most of these don't actually need to be sent because they would be set to their
        // default values anyway. See the SQLite documentation for default values of these PRAGMAs:
        // https://www.sqlite.org/pragma.html
        //
        // However, by inserting into the map here, we can ensure that they're set in the proper
        // order, even if they're overwritten later by their respective setters or
        // directly by `pragma()`

        // Normally, page_size must be set before any other action on the database.
        // Defaults to 4096 for new databases.
        pragmas.insert("page_size".into(), None);

        // locking_mode should be set before journal_mode:
        // https://www.sqlite.org/wal.html#use_of_wal_without_shared_memory
        pragmas.insert("locking_mode".into(), None);

        // Don't set `journal_mode` unless the user requested it.
        // WAL mode is a permanent setting for created databases and changing into or out of it
        // requires an exclusive lock that can't be waited on with `sqlite3_busy_timeout()`.
        // https://github.com/launchbadge/sqlx/pull/1930#issuecomment-1168165414
        pragmas.insert("journal_mode".into(), None);

        // We choose to enable foreign key enforcement by default, though SQLite normally
        // leaves it off for backward compatibility: https://www.sqlite.org/foreignkeys.html#fk_enable
        pragmas.insert("foreign_keys".into(), Some("ON".into()));

        // The `synchronous` pragma defaults to FULL
        // https://www.sqlite.org/compile.html#default_synchronous.
        pragmas.insert("synchronous".into(), None);

        pragmas.insert("auto_vacuum".into(), None);

        // Soft limit on the number of rows that `ANALYZE` touches per index.
        pragmas.insert("analysis_limit".into(), None);

        Self {
            filename: ":memory:".into(),
            in_memory: false,
            read_only: false,
            create_if_missing: false,
            shared_cache: false,
            statement_cache_capacity: 100,
            busy_timeout: Duration::from_secs(5),
            log_settings: Default::default(),
            immutable: false,
            vfs: None,
            pragmas,
            serialized: false,
            thread_name: Arc::new(DebugFn(|id| format!("sqlx-sqlite-worker-{}", id))),
            command_channel_size: 50,
            row_channel_size: 50,
            optimize_on_close: OptimizeOnClose::Disabled,
        }
    }

    /// Set the filename as in-memory. Use the `open_in_memory` method instead, unless you have a very particular use
    /// case.
    pub fn in_memory(mut self, val: bool) -> Self {
        self.in_memory = val;
        self
    }

    /// Sets the name of the database file.
    pub fn filename(mut self, filename: impl AsRef<Path>) -> Self {
        self.filename = filename.as_ref().to_owned();
        self
    }

    /// Set the enforcement of [foreign key constraints](https://www.sqlite.org/pragma.html#pragma_foreign_keys).
    ///
    /// SQLx chooses to enable this by default so that foreign keys function as expected,
    /// compared to other database flavors.
    pub fn foreign_keys(self, on: bool) -> Self {
        self.pragma("foreign_keys", if on { "ON" } else { "OFF" })
    }

    /// Set the [`SQLITE_OPEN_SHAREDCACHE` flag](https://sqlite.org/sharedcache.html).
    ///
    /// By default, this is disabled.
    pub fn shared_cache(mut self, on: bool) -> Self {
        self.shared_cache = on;
        self
    }

    /// Sets the [journal mode](https://www.sqlite.org/pragma.html#pragma_journal_mode) for the database connection.
    ///
    /// Journal modes are ephemeral per connection, with the exception of the
    /// [Write-Ahead Log (WAL) mode](https://www.sqlite.org/wal.html).
    ///
    /// A database created in WAL mode retains the setting and will apply it to all connections
    /// opened against it that don't set a `journal_mode`.
    ///
    /// Opening a connection to a database created in WAL mode with a different `journal_mode` will
    /// erase the setting on the database, requiring an exclusive lock to do so.
    /// You may get a `database is locked` (corresponding to `SQLITE_BUSY`) error if another
    /// connection is accessing the database file at the same time.
    ///
    /// SQLx does not set a journal mode by default, to avoid unintentionally changing a database
    /// into or out of WAL mode.
    ///
    /// The default journal mode for non-WAL databases is `DELETE`, or `MEMORY` for in-memory
    /// databases.
    ///
    /// For consistency, any commands in `sqlx-cli` which create a SQLite database will create it
    /// in WAL mode.
    pub fn journal_mode(self, mode: JournalMode) -> Self {
        self.pragma("journal_mode", mode.as_str())
    }

    /// Sets the [locking mode](https://www.sqlite.org/pragma.html#pragma_locking_mode) for the database connection.
    ///
    /// The default locking mode is NORMAL.
    pub fn locking_mode(self, mode: LockingMode) -> Self {
        self.pragma("locking_mode", mode.as_str())
    }

    /// Sets the [access mode](https://www.sqlite.org/c3ref/open.html) to open the database
    /// for read-only access.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Sets the [access mode](https://www.sqlite.org/c3ref/open.html) to create the database file
    /// if the file does not exist.
    ///
    /// By default, a new file **will not be created** if one is not found.
    pub fn create_if_missing(mut self, create: bool) -> Self {
        self.create_if_missing = create;
        self
    }

    /// Sets the capacity of the connection's statement cache in a number of stored
    /// distinct statements. Caching is handled using LRU, meaning when the
    /// amount of queries hits the defined limit, the oldest statement will get
    /// dropped.
    ///
    /// The default cache capacity is 100 statements.
    pub fn statement_cache_capacity(mut self, capacity: usize) -> Self {
        self.statement_cache_capacity = capacity;
        self
    }

    /// Sets a timeout value to wait when the database is locked, before
    /// returning a busy timeout error.
    ///
    /// The default busy timeout is 5 seconds.
    pub fn busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = timeout;
        self
    }

    /// Sets the [synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous) setting for the database connection.
    ///
    /// The default synchronous settings is FULL. However, if durability is not a concern,
    /// then NORMAL is normally all one needs in WAL mode.
    pub fn synchronous(self, synchronous: Synchronous) -> Self {
        self.pragma("synchronous", synchronous.as_str())
    }

    /// Sets the [auto_vacuum](https://www.sqlite.org/pragma.html#pragma_auto_vacuum) setting for the database connection.
    ///
    /// The default auto_vacuum setting is NONE.
    ///
    /// For existing databases, a change to this value does not take effect unless a
    /// [`VACUUM` command](https://www.sqlite.org/lang_vacuum.html) is executed.
    pub fn auto_vacuum(self, auto_vacuum: AutoVacuum) -> Self {
        self.pragma("auto_vacuum", auto_vacuum.as_str())
    }

    /// Sets the [page_size](https://www.sqlite.org/pragma.html#pragma_page_size) setting for the database connection.
    ///
    /// The default page_size setting is 4096.
    ///
    /// For existing databases, a change to this value does not take effect unless a
    /// [`VACUUM` command](https://www.sqlite.org/lang_vacuum.html) is executed.
    /// However, it cannot be changed in WAL mode.
    pub fn page_size(self, page_size: u32) -> Self {
        self.pragma("page_size", &page_size.to_string())
    }

    /// Sets custom initial pragma for the database connection.
    pub fn pragma(mut self, key: &str, value: &str) -> Self {
        self.pragmas.insert(key.into(), Some(value.into()));
        self
    }

    /// Set to `true` to signal to SQLite that the database file is on read-only media.
    ///
    /// If enabled, SQLite assumes the database file _cannot_ be modified, even by higher
    /// privileged processes, and so disables locking and change detection. This is intended
    /// to improve performance but can produce incorrect query results or errors if the file
    /// _does_ change.
    ///
    /// Note that this is different from the `SQLITE_OPEN_READONLY` flag set by
    /// [`.read_only()`][Self::read_only], though the documentation suggests that this
    /// does _imply_ `SQLITE_OPEN_READONLY`.
    ///
    /// See [`sqlite3_open`](https://www.sqlite.org/capi3ref.html#sqlite3_open) (subheading
    /// "URI Filenames") for details.
    pub fn immutable(mut self, immutable: bool) -> Self {
        self.immutable = immutable;
        self
    }

    /// Sets the [threading mode](https://www.sqlite.org/threadsafe.html) for the database connection.
    ///
    /// The default setting is `false` corresponding to using `OPEN_NOMUTEX`.
    /// If set to `true` then `OPEN_FULLMUTEX`.
    ///
    /// See [open](https://www.sqlite.org/c3ref/open.html) for more details.
    ///
    /// ### Note
    /// Setting this to `true` may help if you are getting access violation errors or segmentation
    /// faults, but will also incur a significant performance penalty. You should leave this
    /// set to `false` if at all possible.
    ///
    /// If you do end up needing to set this to `true` for some reason, please
    /// [open an issue](https://github.com/launchbadge/sqlx/issues/new/choose) as this may indicate
    /// a concurrency bug in SQLx. Please provide clear instructions for reproducing the issue,
    /// including a sample database schema if applicable.
    pub fn serialized(mut self, serialized: bool) -> Self {
        self.serialized = serialized;
        self
    }

    /// Provide a callback to generate the name of the background worker thread.
    ///
    /// The value passed to the callback is an auto-incremented integer for use as the thread ID.
    pub fn thread_name(
        mut self,
        generator: impl Fn(u64) -> String + Send + Sync + 'static,
    ) -> Self {
        self.thread_name = Arc::new(DebugFn(generator));
        self
    }

    /// Set the maximum number of commands to buffer for the worker thread before backpressure is
    /// applied.
    ///
    /// Given that most commands sent to the worker thread involve waiting for a result,
    /// the command channel is unlikely to fill up unless a lot queries are executed in a short
    /// period but cancelled before their full resultsets are returned.
    pub fn command_buffer_size(mut self, size: usize) -> Self {
        self.command_channel_size = size;
        self
    }

    /// Set the maximum number of rows to buffer back to the calling task when a query is executed.
    ///
    /// If the calling task cannot keep up, backpressure will be applied to the worker thread
    /// in order to limit CPU and memory usage.
    pub fn row_buffer_size(mut self, size: usize) -> Self {
        self.row_channel_size = size;
        self
    }

    /// Sets the [`vfs`](https://www.sqlite.org/vfs.html) parameter of the database connection.
    ///
    /// The default value is empty, and sqlite will use the default VFS object depending on the
    /// operating system.
    pub fn vfs(mut self, vfs_name: &str) -> Self {
        self.vfs = Some(vfs_name.into());
        self
    }

    /// Execute `PRAGMA optimize;` on the SQLite connection before closing.
    ///
    /// The SQLite manual recommends using this for long-lived databases.
    ///
    /// This will collect and store statistics about the layout of data in your tables to help the query planner make
    /// better decisions. Over the connection's lifetime, the query planner will make notes about which tables could use
    /// up-to-date statistics so this command doesn't have to scan the whole database every time. Thus, the best time to
    /// execute this is on connection close.
    ///
    /// `analysis_limit` sets a soft limit on the maximum number of rows to scan per index. It is equivalent to setting
    /// [`Self::analysis_limit`] but only takes effect for the `PRAGMA optimize;` call and does not affect the behavior
    /// of any `ANALYZE` statements made during the connection's lifetime.
    ///
    /// If not `None`, the `analysis_limit` here overrides the global `analysis_limit` setting, but only for the `PRAGMA
    /// optimize;` call.
    ///
    /// Not enabled by default.
    ///
    /// See [the SQLite manual](https://www.sqlite.org/lang_analyze.html#automatically_running_analyze) for details.
    pub fn optimize_on_close(
        mut self,
        enabled: bool,
        analysis_limit: impl Into<Option<u32>>,
    ) -> Self {
        self.optimize_on_close = if enabled {
            OptimizeOnClose::Enabled {
                analysis_limit: (analysis_limit.into()),
            }
        } else {
            OptimizeOnClose::Disabled
        };
        self
    }

    /// Set a soft limit on the number of rows that `ANALYZE` touches per index.
    ///
    /// This also affects `PRAGMA optimize` which is set by [Self::optimize_on_close].
    ///
    /// The value recommended by SQLite is `400`. There is no default.
    ///
    /// See [the SQLite manual](https://www.sqlite.org/lang_analyze.html#approx) for details.
    pub fn analysis_limit(mut self, limit: Option<u32>) -> Self {
        if let Some(limit) = limit {
            return self.pragma("analysis_limit", &limit.to_string());
        }
        self.pragmas.insert("analysis_limit".into(), None);
        self
    }

    pub fn log_statements(mut self, level: LevelFilter) -> Self {
        self.log_settings.log_statements(level);
        self
    }

    pub fn log_slow_statements(mut self, level: LevelFilter, duration: Duration) -> Self {
        self.log_settings.log_slow_statements(level, duration);
        self
    }

    /// Collect all `PRAMGA` commands into a single string
    pub(crate) fn pragma_string(&self) -> String {
        let mut string = String::new();
        for (key, opt_value) in &self.pragmas {
            if let Some(value) = opt_value {
                write!(string, "PRAGMA {} = {}; ", key, value).ok();
            }
        }
        string
    }

    pub(crate) async fn connect(&self) -> Result<Connection> {
        let mut conn = Connection::establish(self).await?;
        // Execute PRAGMAs
        conn.execute(&*self.pragma_string()).await?;
        Ok(conn)
    }

    pub fn with_pool(&self) -> PoolOptions {
        PoolOptions::new(self.clone())
    }

    pub(crate) fn configure_in_memory(self) -> Self {
        let seqno = IN_MEMORY_DB_SEQ.fetch_add(1, Ordering::Relaxed);
        self.in_memory(true)
            .shared_cache(true)
            .filename(format!("file:musqlite-in-memory-{}", seqno))
    }

    /// Open a file
    pub async fn open(self, filename: impl AsRef<Path>) -> Result<pool::Pool> {
        self.filename(filename).with_pool().connect().await
    }

    /// Open an in-memory database
    pub async fn open_in_memory(self) -> Result<pool::Pool> {
        self.configure_in_memory().with_pool().connect().await
    }
}
