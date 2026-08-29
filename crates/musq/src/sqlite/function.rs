//! User-defined scalar functions and collations.

use std::{
    cmp::Ordering,
    ffi::{CString, c_void},
    fmt::{self, Debug, Formatter},
    os::raw::{c_char, c_int},
    panic::{AssertUnwindSafe, catch_unwind},
    slice, str,
    sync::Arc,
};

use bytes::Bytes;
use libsqlite3_sys::{
    self as ffi_sys, SQLITE_BLOB, SQLITE_DETERMINISTIC, SQLITE_DIRECTONLY, SQLITE_FLOAT,
    SQLITE_INNOCUOUS, SQLITE_INTEGER, SQLITE_TEXT, SQLITE_UTF8, sqlite3, sqlite3_context,
    sqlite3_value,
};

use crate::{Result, Value, error::Error, sqlite::error::SqliteError};

/// Flags for a user-defined scalar function.
///
/// `direct_only` is the default. With trusted schema off, only an
/// `innocuous` function may appear in an index, view, or trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFlags {
    /// SQLite may assume the function is deterministic.
    pub deterministic: bool,
    /// The function may not run from schema objects such as indexes.
    pub direct_only: bool,
    /// The function is safe to use in schema objects when trusted schema is off.
    pub innocuous: bool,
}

impl Default for FunctionFlags {
    fn default() -> Self {
        Self {
            deterministic: false,
            direct_only: true,
            innocuous: false,
        }
    }
}

/// Scalar function implementation stored as an `Arc`.
type ScalarFn = dyn Fn(&[Value]) -> Result<Value> + Send + Sync;
/// Collation implementation stored as an `Arc`.
type CollationFn = dyn Fn(&str, &str) -> Ordering + Send + Sync;

impl FunctionFlags {
    /// SQLite `eTextRep` flags for this function.
    fn sqlite_flags(self) -> i32 {
        let mut flags = SQLITE_UTF8;
        if self.deterministic {
            flags |= SQLITE_DETERMINISTIC;
        }
        if self.direct_only {
            flags |= SQLITE_DIRECTONLY;
        }
        if self.innocuous {
            flags |= SQLITE_INNOCUOUS;
        }
        flags
    }
}

/// A scalar function registered on every new connection.
#[derive(Clone)]
pub struct RegisteredFunction {
    /// SQL function name.
    name: String,
    /// Argument count, or `-1` for a variable-argument function.
    n_args: i32,
    /// SQLite function flags.
    flags: FunctionFlags,
    /// Rust implementation.
    func: Arc<ScalarFn>,
}

impl Debug for RegisteredFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredFunction")
            .field("name", &self.name)
            .field("n_args", &self.n_args)
            .field("flags", &self.flags)
            .finish()
    }
}

impl RegisteredFunction {
    /// Build a registered scalar function.
    pub fn new<F>(name: String, n_args: i32, flags: FunctionFlags, func: F) -> Self
    where
        F: Fn(&[Value]) -> Result<Value> + Send + Sync + 'static,
    {
        Self {
            name,
            n_args,
            flags,
            func: Arc::new(func),
        }
    }
}

/// A collation registered on every new connection.
#[derive(Clone)]
pub struct RegisteredCollation {
    /// SQL collation name.
    name: String,
    /// Rust implementation.
    func: Arc<CollationFn>,
}

impl Debug for RegisteredCollation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredCollation")
            .field("name", &self.name)
            .finish()
    }
}

impl RegisteredCollation {
    /// Build a registered collation.
    pub fn new<F>(name: String, func: F) -> Self
    where
        F: Fn(&str, &str) -> Ordering + Send + Sync + 'static,
    {
        Self {
            name,
            func: Arc::new(func),
        }
    }
}

/// Register scalar functions and collations on a live connection.
pub fn register_all(
    db: *mut sqlite3,
    functions: &[Arc<RegisteredFunction>],
    collations: &[Arc<RegisteredCollation>],
) -> Result<()> {
    for function in functions {
        register_function(db, function)?;
    }
    for collation in collations {
        register_collation(db, collation)?;
    }
    Ok(())
}

/// Register one scalar function. SQLite owns a clone of the `Arc` until close.
fn register_function(db: *mut sqlite3, function: &RegisteredFunction) -> Result<()> {
    let name = CString::new(function.name.as_str()).map_err(|_| {
        Error::Configuration(format!(
            "function name contains nul bytes: {}",
            function.name
        ))
    })?;
    let app = Box::into_raw(Box::new(Arc::clone(&function.func)));
    let rc = unsafe {
        ffi_sys::sqlite3_create_function_v2(
            db,
            name.as_ptr(),
            function.n_args,
            function.flags.sqlite_flags(),
            app.cast(),
            Some(scalar_entry),
            None,
            None,
            Some(destroy_scalar),
        )
    };
    if rc != ffi_sys::SQLITE_OK {
        unsafe { drop(Box::from_raw(app)) };
        return Err(SqliteError::new(db).into());
    }
    Ok(())
}

/// Register one collation. SQLite owns a clone of the `Arc` until close.
fn register_collation(db: *mut sqlite3, collation: &RegisteredCollation) -> Result<()> {
    let name = CString::new(collation.name.as_str()).map_err(|_| {
        Error::Configuration(format!(
            "collation name contains nul bytes: {}",
            collation.name
        ))
    })?;
    let app = Box::into_raw(Box::new(Arc::clone(&collation.func)));
    let rc = unsafe {
        ffi_sys::sqlite3_create_collation_v2(
            db,
            name.as_ptr(),
            SQLITE_UTF8,
            app.cast(),
            Some(collation_entry),
            Some(destroy_collation),
        )
    };
    if rc != ffi_sys::SQLITE_OK {
        unsafe { drop(Box::from_raw(app)) };
        return Err(SqliteError::new(db).into());
    }
    Ok(())
}

/// Drop the boxed scalar `Arc` when SQLite unregisters the function.
unsafe extern "C" fn destroy_scalar(p: *mut c_void) {
    if !p.is_null() {
        drop(unsafe { Box::from_raw(p.cast::<Arc<ScalarFn>>()) });
    }
}

/// Drop the boxed collation `Arc` when SQLite unregisters the collation.
unsafe extern "C" fn destroy_collation(p: *mut c_void) {
    if !p.is_null() {
        drop(unsafe { Box::from_raw(p.cast::<Arc<CollationFn>>()) });
    }
}

/// SQLite scalar-function entry point. Catches panics and maps Rust errors.
unsafe extern "C" fn scalar_entry(
    ctx: *mut sqlite3_context,
    argc: c_int,
    argv: *mut *mut sqlite3_value,
) {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let func = unsafe { &*(ffi_sys::sqlite3_user_data(ctx) as *const Arc<ScalarFn>) };
        let args = unsafe { values_from_argv(argc, argv) };
        func(&args)
    }));
    match outcome {
        Ok(Ok(value)) => unsafe { result_value(ctx, &value) },
        Ok(Err(error)) => unsafe { result_error(ctx, &error.to_string()) },
        Err(_) => unsafe { result_error(ctx, "musq: function panicked") },
    }
}

/// SQLite collation entry point. Catches panics and returns `0` if one occurs.
unsafe extern "C" fn collation_entry(
    p: *mut c_void,
    n1: c_int,
    p1: *const c_void,
    n2: c_int,
    p2: *const c_void,
) -> c_int {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let func = unsafe { &*(p as *const Arc<CollationFn>) };
        let left = unsafe { bytes_from_ptr(p1, n1) };
        let right = unsafe { bytes_from_ptr(p2, n2) };
        match (str::from_utf8(left), str::from_utf8(right)) {
            (Ok(left), Ok(right)) => func(left, right),
            _ => left.cmp(right),
        }
    }));
    match outcome {
        Ok(Ordering::Less) => -1,
        Ok(Ordering::Equal) => 0,
        Ok(Ordering::Greater) => 1,
        Err(_) => 0,
    }
}

/// Read function arguments from SQLite value pointers.
unsafe fn values_from_argv(argc: c_int, argv: *mut *mut sqlite3_value) -> Vec<Value> {
    let n = usize::try_from(argc).unwrap_or(0);
    let ptrs = if n == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(argv, n) }
    };
    ptrs.iter()
        .map(|ptr| unsafe { value_from_sqlite(*ptr) })
        .collect()
}

/// Convert one `sqlite3_value` to [`Value`].
unsafe fn value_from_sqlite(value: *mut sqlite3_value) -> Value {
    match unsafe { ffi_sys::sqlite3_value_type(value) } {
        SQLITE_INTEGER => Value::Integer {
            value: unsafe { ffi_sys::sqlite3_value_int64(value) },
            type_info: None,
        },
        SQLITE_FLOAT => Value::Double {
            value: unsafe { ffi_sys::sqlite3_value_double(value) },
            type_info: None,
        },
        SQLITE_TEXT => {
            let len = unsafe { ffi_sys::sqlite3_value_bytes(value) };
            let ptr = unsafe { ffi_sys::sqlite3_value_text(value) };
            let bytes = if ptr.is_null() {
                Bytes::new()
            } else {
                Bytes::copy_from_slice(unsafe {
                    slice::from_raw_parts(ptr, usize::try_from(len).unwrap_or(0))
                })
            };
            Value::Text {
                value: bytes,
                type_info: None,
            }
        }
        SQLITE_BLOB => {
            let len = unsafe { ffi_sys::sqlite3_value_bytes(value) };
            let ptr = unsafe { ffi_sys::sqlite3_value_blob(value) };
            let bytes = if ptr.is_null() {
                Bytes::new()
            } else {
                Bytes::copy_from_slice(unsafe {
                    slice::from_raw_parts(ptr.cast::<u8>(), usize::try_from(len).unwrap_or(0))
                })
            };
            Value::Blob {
                value: bytes,
                type_info: None,
            }
        }
        _ => Value::Null { type_info: None },
    }
}

/// Write a Musq [`Value`] as the SQL function result.
unsafe fn result_value(ctx: *mut sqlite3_context, value: &Value) {
    match value {
        Value::Null { .. } => unsafe { ffi_sys::sqlite3_result_null(ctx) },
        Value::Integer { value, .. } => unsafe { ffi_sys::sqlite3_result_int64(ctx, *value) },
        Value::Double { value, .. } => unsafe { ffi_sys::sqlite3_result_double(ctx, *value) },
        Value::Text { value, .. } => unsafe {
            ffi_sys::sqlite3_result_text64(
                ctx,
                value.as_ptr().cast::<c_char>(),
                value.len() as u64,
                ffi_sys::SQLITE_TRANSIENT(),
                SQLITE_UTF8 as u8,
            );
        },
        Value::Blob { value, .. } => unsafe {
            ffi_sys::sqlite3_result_blob64(
                ctx,
                value.as_ptr().cast::<c_void>(),
                value.len() as u64,
                ffi_sys::SQLITE_TRANSIENT(),
            );
        },
    }
}

/// Report a UTF-8 error message as the SQL function result.
unsafe fn result_error(ctx: *mut sqlite3_context, message: &str) {
    let cstr =
        CString::new(message).unwrap_or_else(|_| CString::new("musq function error").unwrap());
    unsafe { ffi_sys::sqlite3_result_error(ctx, cstr.as_ptr(), -1) }
}

/// Borrow bytes from a collation argument pointer.
unsafe fn bytes_from_ptr<'a>(ptr: *const c_void, len: c_int) -> &'a [u8] {
    if ptr.is_null() || len <= 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(ptr.cast::<u8>(), len as usize) }
    }
}
