use std::{
    ffi::CString,
    io,
    ptr::{null, null_mut},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use crate::sqlite::ffi;
use libsqlite3_sys::{
    SQLITE_OPEN_CREATE, SQLITE_OPEN_FULLMUTEX, SQLITE_OPEN_MEMORY, SQLITE_OPEN_NOMUTEX,
    SQLITE_OPEN_PRIVATECACHE, SQLITE_OPEN_READONLY, SQLITE_OPEN_READWRITE, SQLITE_OPEN_SHAREDCACHE,
};

use crate::{
    Error, Musq,
    sqlite::connection::{ConnectionState, LogSettings, StatementCache, handle::ConnectionHandle},
};

static THREAD_ID: AtomicU64 = AtomicU64::new(0);

pub struct EstablishParams {
    filename: CString,
    open_flags: i32,
    busy_timeout: Duration,
    log_settings: LogSettings,
    statement_cache_capacity: usize,
    pub(crate) thread_name: String,
    pub(crate) command_channel_size: usize,
}

impl EstablishParams {
    pub fn from_options(options: &Musq) -> Result<Self, Error> {
        let mut filename = options
            .filename
            .to_str()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "filename passed to SQLite must be valid UTF-8",
                )
            })?
            .to_owned();

        // By default, we connect to an in-memory database.
        // [SQLITE_OPEN_NOMUTEX] will instruct [sqlite3_open_v2] to return an error if it
        // cannot satisfy our wish for a thread-safe, lock-free connection object

        let mut flags = if options.serialized {
            SQLITE_OPEN_FULLMUTEX
        } else {
            SQLITE_OPEN_NOMUTEX
        };

        flags |= if options.read_only {
            SQLITE_OPEN_READONLY
        } else if options.create_if_missing {
            SQLITE_OPEN_CREATE | SQLITE_OPEN_READWRITE
        } else {
            SQLITE_OPEN_READWRITE
        };

        if options.in_memory {
            flags |= SQLITE_OPEN_MEMORY;
        }

        flags |= if options.shared_cache {
            SQLITE_OPEN_SHAREDCACHE
        } else {
            SQLITE_OPEN_PRIVATECACHE
        };

        let mut query_params: Vec<String> = vec![];

        if options.immutable {
            query_params.push("immutable=true".into())
        }

        if let Some(vfs) = &options.vfs {
            query_params.push(format!("vfs={vfs}"))
        }

        if !query_params.is_empty() {
            filename = format!("file:{}?{}", filename, query_params.join("&"));
            flags |= libsqlite3_sys::SQLITE_OPEN_URI;
        }

        let filename = CString::new(filename).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "filename passed to SQLite must not contain nul bytes",
            )
        })?;

        Ok(Self {
            filename,
            open_flags: flags,
            busy_timeout: options.busy_timeout,
            log_settings: options.log_settings.clone(),
            statement_cache_capacity: options.statement_cache_capacity,
            thread_name: (options.thread_name)(THREAD_ID.fetch_add(1, Ordering::AcqRel)),
            command_channel_size: options.command_channel_size,
        })
    }

    /// Establish a new SQLite connection.
    ///
    /// The configured busy timeout is converted to milliseconds for
    /// [`sqlite3_busy_timeout`]. If the duration exceeds `i32::MAX`
    /// milliseconds, it is clamped to `i32::MAX`.
    pub(crate) fn establish(&self) -> Result<ConnectionState, Error> {
        let mut handle = null_mut();

        // <https://www.sqlite.org/c3ref/open.html>
        let open_res = ffi::open_v2(self.filename.as_ptr(), &mut handle, self.open_flags, null());

        if handle.is_null() {
            // Failed to allocate memory
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "SQLite is unable to allocate memory to hold the sqlite3 object",
            )));
        }

        if let Err(e) = open_res {
            // handle may already be closed inside `open_v2`
            return Err(Error::Sqlite(e));
        }

        // SAFE: tested for NULL just above and open_v2 succeeded
        let handle = unsafe { ConnectionHandle::new(handle) };

        // Enable extended result codes
        // https://www.sqlite.org/c3ref/extended_result_codes.html
        // NOTE: ignore the failure here
        let _ = ffi::extended_result_codes(handle.as_ptr(), 1);

        // Configure a busy timeout
        // This causes SQLite to automatically sleep in increasing intervals until the time
        // when there is something locked during [sqlite3_step].
        //
        // We also need to convert the u128 value to i32. If the value overflows,
        // we clamp to `i32::MAX` to comply with SQLite's API.
        let ms = match i32::try_from(self.busy_timeout.as_millis()) {
            Ok(ms) => ms,
            Err(_) => i32::MAX,
        };

        ffi::busy_timeout(handle.as_ptr(), ms).map_err(Error::Sqlite)?;

        Ok(ConnectionState {
            handle,
            statements: StatementCache::new(self.statement_cache_capacity),
            transaction_depth: 0,
            log_settings: self.log_settings.clone(),
            progress_handler_callback: None,
        })
    }
}
