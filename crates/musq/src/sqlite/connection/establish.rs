use std::{
    ffi::CString,
    io,
    ptr::{null, null_mut},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use libsqlite3_sys::{
    SQLITE_OPEN_CREATE, SQLITE_OPEN_FULLMUTEX, SQLITE_OPEN_MEMORY, SQLITE_OPEN_NOMUTEX,
    SQLITE_OPEN_PRIVATECACHE, SQLITE_OPEN_READONLY, SQLITE_OPEN_READWRITE, SQLITE_OPEN_SHAREDCACHE,
};

use crate::{
    Error, Musq, Result,
    sqlite::{
        connection::{ConnectionState, LogSettings, StatementCache, handle::ConnectionHandle},
        ffi,
    },
};

/// Monotonic counter for naming worker threads.
static THREAD_ID: AtomicU64 = AtomicU64::new(0);

/// Derived parameters for establishing a SQLite connection.
pub struct EstablishParams {
    /// Database filename as a C-compatible string.
    filename: CString,
    /// SQLite open flags.
    open_flags: i32,
    /// Busy timeout to apply after connection.
    busy_timeout: Duration,
    /// Logging configuration.
    log_settings: LogSettings,
    /// Statement cache capacity.
    statement_cache_capacity: usize,
    /// Floating-point text precision to apply with SQLITE_DBCONFIG_FP_DIGITS.
    floating_point_text_digits: Option<u8>,
    /// Parser stack depth limit to apply with sqlite3_limit().
    parser_depth_limit: Option<i32>,
    /// Thread name for connection worker.
    pub(crate) thread_name: String,
    /// Size of the command channel to the worker.
    pub(crate) command_channel_size: usize,
}

impl EstablishParams {
    /// Build connection parameters from user options.
    pub fn from_options(options: &Musq) -> Result<Self> {
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

        let floating_point_text_digits =
            validate_floating_point_text_digits(options.floating_point_text_digits)?;
        let parser_depth_limit = validate_parser_depth_limit(options.parser_depth_limit)?;

        Ok(Self {
            filename,
            open_flags: flags,
            busy_timeout: options.busy_timeout,
            log_settings: options.log_settings.clone(),
            statement_cache_capacity: options.statement_cache_capacity,
            floating_point_text_digits,
            parser_depth_limit,
            thread_name: (options.thread_name)(THREAD_ID.fetch_add(1, Ordering::AcqRel)),
            command_channel_size: options.command_channel_size,
        })
    }

    /// Establish a new SQLite connection.
    ///
    /// The configured busy timeout is converted to milliseconds for
    /// [`sqlite3_busy_timeout`]. If the duration exceeds `i32::MAX`
    /// milliseconds, it is clamped to `i32::MAX`.
    pub(crate) fn establish(&self) -> Result<ConnectionState> {
        #[cfg(feature = "vec")]
        ffi::register_vec()?;

        let mut handle = null_mut();

        // <https://www.sqlite.org/c3ref/open.html>
        let open_res = ffi::open_v2(self.filename.as_ptr(), &mut handle, self.open_flags, null());

        if let Err(e) = open_res {
            // handle is already closed inside `open_v2`
            return Err(e.into());
        }

        if handle.is_null() {
            // Failed to allocate memory
            return Err(Error::Io(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "SQLite is unable to allocate memory to hold the sqlite3 object",
            )));
        }

        // SAFE: tested for NULL just above and open_v2 succeeded
        let handle = unsafe { ConnectionHandle::new(handle) };

        // Enable extended result codes
        // https://www.sqlite.org/c3ref/extended_result_codes.html
        // On failure return the sqlite error for visibility
        ffi::extended_result_codes(handle.as_ptr(), 1).map_err(Error::from)?;

        // Configure a busy timeout
        // This causes SQLite to automatically sleep in increasing intervals until the time
        // when there is something locked during [sqlite3_step].
        //
        // We also need to convert the u128 value to i32. If the value overflows,
        // we clamp to `i32::MAX` to comply with SQLite's API.
        let ms = i32::try_from(self.busy_timeout.as_millis()).unwrap_or(i32::MAX);

        ffi::busy_timeout(handle.as_ptr(), ms).map_err(Error::from)?;

        if let Some(digits) = self.floating_point_text_digits {
            let configured = ffi::db_config_fp_digits(handle.as_ptr(), i32::from(digits))
                .map_err(Error::from)?;
            if configured != i32::from(digits) {
                return Err(Error::Protocol(format!(
                    "SQLite reported floating point text digits {configured} after setting {digits}"
                )));
            }
        }

        if let Some(limit) = self.parser_depth_limit {
            ffi::limit(
                handle.as_ptr(),
                libsqlite3_sys::SQLITE_LIMIT_PARSER_DEPTH,
                limit,
            );
        }

        Ok(ConnectionState {
            handle,
            statements: StatementCache::new(self.statement_cache_capacity),
            transaction_depth: 0,
            log_settings: self.log_settings.clone(),
        })
    }
}

/// Validate the configured floating-point text precision.
fn validate_floating_point_text_digits(digits: Option<u8>) -> Result<Option<u8>> {
    if let Some(digits) = digits
        && !(4..=23).contains(&digits)
    {
        return Err(Error::Protocol(format!(
            "floating_point_text_digits must be between 4 and 23, got {digits}"
        )));
    }

    Ok(digits)
}

/// Validate the configured parser depth limit.
fn validate_parser_depth_limit(limit: Option<u32>) -> Result<Option<i32>> {
    let Some(limit) = limit else {
        return Ok(None);
    };

    if limit == 0 {
        return Err(Error::Protocol(
            "parser_depth_limit must be greater than zero".into(),
        ));
    }

    i32::try_from(limit)
        .map(Some)
        .map_err(|_| Error::Protocol(format!("parser_depth_limit must fit into i32, got {limit}")))
}
