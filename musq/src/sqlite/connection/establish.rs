use std::{
    ffi::CString,
    io,
    ptr::{null, null_mut},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use libsqlite3_sys::{
    SQLITE_OK, SQLITE_OPEN_CREATE, SQLITE_OPEN_FULLMUTEX, SQLITE_OPEN_MEMORY, SQLITE_OPEN_NOMUTEX,
    SQLITE_OPEN_PRIVATECACHE, SQLITE_OPEN_READONLY, SQLITE_OPEN_READWRITE, SQLITE_OPEN_SHAREDCACHE,
    sqlite3_busy_timeout, sqlite3_extended_result_codes, sqlite3_open_v2,
};

use crate::{
    Error, Musq,
    sqlite::{
        SqliteError,
        connection::{ConnectionState, LogSettings, StatementCache, handle::ConnectionHandle},
    },
};

static THREAD_ID: AtomicU64 = AtomicU64::new(0);

pub struct EstablishParams {
    filename: CString,
    open_flags: i32,
    busy_timeout: Duration,
    log_settings: LogSettings,
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
            thread_name: (options.thread_name)(THREAD_ID.fetch_add(1, Ordering::AcqRel)),
            command_channel_size: options.command_channel_size,
        })
    }

    pub(crate) fn establish(&self) -> Result<ConnectionState, Error> {
        let mut handle = null_mut();

        // <https://www.sqlite.org/c3ref/open.html>
        let mut status = unsafe {
            sqlite3_open_v2(self.filename.as_ptr(), &mut handle, self.open_flags, null())
        };

        if handle.is_null() {
            // Failed to allocate memory
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "SQLite is unable to allocate memory to hold the sqlite3 object",
            )));
        }

        // SAFE: tested for NULL just above
        // This allows any returns below to close this handle with RAII
        let handle = unsafe { ConnectionHandle::new(handle) };

        if status != SQLITE_OK {
            return Err(Error::Sqlite(SqliteError::new(handle.as_ptr())));
        }

        // Enable extended result codes
        // https://www.sqlite.org/c3ref/extended_result_codes.html
        unsafe {
            // NOTE: ignore the failure here
            sqlite3_extended_result_codes(handle.as_ptr(), 1);
        }

        // Configure a busy timeout
        // This causes SQLite to automatically sleep in increasing intervals until the time
        // when there is something locked during [sqlite3_step].
        //
        // We also need to convert the u128 value to i32, checking we're not overflowing.
        let ms = i32::try_from(self.busy_timeout.as_millis())
            .expect("Given busy timeout value is too big.");

        status = unsafe { sqlite3_busy_timeout(handle.as_ptr(), ms) };

        if status != SQLITE_OK {
            return Err(Error::Sqlite(SqliteError::new(handle.as_ptr())));
        }

        Ok(ConnectionState {
            handle,
            statements: StatementCache::new(),
            transaction_depth: 0,
            log_settings: self.log_settings.clone(),
            progress_handler_callback: None,
        })
    }
}
