use std::{
    ffi::CString,
    io,
    ptr::{null, null_mut},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use libsqlite3_sys::{
    SQLITE_OPEN_CREATE, SQLITE_OPEN_EXRESCODE, SQLITE_OPEN_FULLMUTEX, SQLITE_OPEN_NOMUTEX,
    SQLITE_OPEN_READONLY, SQLITE_OPEN_READWRITE,
};

use crate::{
    Error, Musq, Result,
    sqlite::{
        connection::{ConnectionState, LogSettings, StatementCache, handle::ConnectionHandle},
        ffi,
        function::{RegisteredCollation, RegisteredFunction, register_all},
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
    /// Whether double-quoted string literals are accepted.
    double_quoted_strings: bool,
    /// Whether schema objects are trusted for non-innocuous functions.
    trusted_schema: bool,
    /// Whether SQLite defensive mode is enabled.
    defensive: bool,
    /// Optional per-statement runtime limit.
    pub(crate) statement_timeout: Option<Duration>,
    /// Thread name for connection worker.
    pub(crate) thread_name: String,
    /// Size of the command channel to the worker.
    pub(crate) command_channel_size: usize,
    /// Scalar functions to register after open.
    functions: Vec<Arc<RegisteredFunction>>,
    /// Collations to register after open.
    collations: Vec<Arc<RegisteredCollation>>,
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
        flags |= SQLITE_OPEN_EXRESCODE;

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
            double_quoted_strings: options.double_quoted_strings,
            trusted_schema: options.trusted_schema,
            defensive: options.defensive,
            statement_timeout: options.statement_timeout,
            thread_name: (options.thread_name)(THREAD_ID.fetch_add(1, Ordering::AcqRel)),
            command_channel_size: options.command_channel_size,
            functions: options.functions.clone(),
            collations: options.collations.clone(),
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
        // SAFETY: `filename` is a valid CString; `handle` is a local out-parameter.
        let open_res =
            unsafe { ffi::open_v2(self.filename.as_ptr(), &mut handle, self.open_flags, null()) };

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

        // Configure a busy timeout
        // This causes SQLite to automatically sleep in increasing intervals until the time
        // when there is something locked during [sqlite3_step].
        //
        // We also need to convert the u128 value to i32. If the value overflows,
        // we clamp to `i32::MAX` to comply with SQLite's API.
        let ms = i32::try_from(self.busy_timeout.as_millis()).unwrap_or(i32::MAX);

        // SAFETY: `handle` is a newly opened live connection.
        unsafe { ffi::busy_timeout(handle.as_ptr(), ms) }.map_err(Error::from)?;

        set_db_config_flag(
            &handle,
            ffi::DbConfigIntOp::DqsDdl,
            self.double_quoted_strings,
        )?;
        set_db_config_flag(
            &handle,
            ffi::DbConfigIntOp::DqsDml,
            self.double_quoted_strings,
        )?;
        set_db_config_flag(
            &handle,
            ffi::DbConfigIntOp::TrustedSchema,
            self.trusted_schema,
        )?;
        set_db_config_flag(&handle, ffi::DbConfigIntOp::Defensive, self.defensive)?;

        if let Some(digits) = self.floating_point_text_digits {
            let configured = unsafe {
                ffi::db_config_int(
                    handle.as_ptr(),
                    ffi::DbConfigIntOp::FpDigits,
                    i32::from(digits),
                )
            }
            .map_err(Error::from)?;
            if configured != i32::from(digits) {
                return Err(Error::Protocol(format!(
                    "SQLite reported floating point text digits {configured} after setting {digits}"
                )));
            }
        }

        if let Some(limit) = self.parser_depth_limit {
            unsafe {
                ffi::limit(
                    handle.as_ptr(),
                    libsqlite3_sys::SQLITE_LIMIT_PARSER_DEPTH,
                    limit,
                )
            };
        }

        register_all(handle.as_ptr(), &self.functions, &self.collations)?;

        Ok(ConnectionState {
            handle,
            statements: StatementCache::new(self.statement_cache_capacity),
            transaction_depth: 0,
            log_settings: self.log_settings.clone(),
        })
    }
}

/// Apply a boolean `sqlite3_db_config` switch and require SQLite to report the same value.
fn set_db_config_flag(handle: &ConnectionHandle, op: ffi::DbConfigIntOp, on: bool) -> Result<()> {
    let value = i32::from(on);
    let configured =
        unsafe { ffi::db_config_int(handle.as_ptr(), op, value) }.map_err(Error::from)?;
    if configured != value {
        return Err(Error::Protocol(format!(
            "SQLite reported db_config {configured} after setting {value}"
        )));
    }
    Ok(())
}

/// Validate the configured floating-point text precision.
fn validate_floating_point_text_digits(digits: Option<u8>) -> Result<Option<u8>> {
    if let Some(digits) = digits
        && !(4..=23).contains(&digits)
    {
        return Err(Error::Configuration(format!(
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
        return Err(Error::Configuration(
            "parser_depth_limit must be greater than zero".into(),
        ));
    }

    i32::try_from(limit).map(Some).map_err(|_| {
        Error::Configuration(format!("parser_depth_limit must fit into i32, got {limit}"))
    })
}
