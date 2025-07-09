use std::{ptr::NonNull, slice::from_raw_parts, str::from_utf8, sync::Arc};

use libsqlite3_sys::{
    sqlite3_value, sqlite3_value_blob, sqlite3_value_bytes, sqlite3_value_double,
    sqlite3_value_dup, sqlite3_value_free, sqlite3_value_int, sqlite3_value_int64,
    sqlite3_value_type, SQLITE_NULL,
};

use crate::{error::DecodeError, sqlite::type_info::SqliteDataType};

#[derive(Clone)]
pub struct Value {
    pub(crate) handle: Arc<ValueHandle>,
    pub(crate) type_info: SqliteDataType,
}

pub(crate) struct ValueHandle(NonNull<sqlite3_value>);

// SAFE: only protected value objects are stored in SqliteValue
unsafe impl Send for ValueHandle {}
unsafe impl Sync for ValueHandle {}

impl Value {
    pub(crate) unsafe fn new(value: *mut sqlite3_value, type_info: SqliteDataType) -> Self {
        debug_assert!(!value.is_null());

        Self {
            type_info,
            handle: Arc::new(ValueHandle(unsafe {
                NonNull::new_unchecked(sqlite3_value_dup(value))
            })),
        }
    }

    fn type_info_opt(&self) -> Option<SqliteDataType> {
        let dt = SqliteDataType::from_code(unsafe { sqlite3_value_type(self.handle.0.as_ptr()) });
        if let SqliteDataType::Null = dt {
            None
        } else {
            Some(dt)
        }
    }

    pub fn int(&self) -> i32 {
        unsafe { sqlite3_value_int(self.handle.0.as_ptr()) }
    }

    pub fn int64(&self) -> i64 {
        unsafe { sqlite3_value_int64(self.handle.0.as_ptr()) }
    }

    pub fn double(&self) -> f64 {
        unsafe { sqlite3_value_double(self.handle.0.as_ptr()) }
    }

    pub fn blob(&self) -> &[u8] {
        let len = unsafe { sqlite3_value_bytes(self.handle.0.as_ptr()) } as usize;

        if len == 0 {
            // empty blobs are NULL so just return an empty slice
            return &[];
        }

        let ptr = unsafe { sqlite3_value_blob(self.handle.0.as_ptr()) } as *const u8;
        debug_assert!(!ptr.is_null());

        unsafe { from_raw_parts(ptr, len) }
    }

    pub fn text(&self) -> Result<&str, DecodeError> {
        from_utf8(self.blob()).map_err(|e| DecodeError::Conversion(e.to_string()))
    }

    pub fn type_info(&self) -> SqliteDataType {
        self.type_info_opt().unwrap_or(self.type_info)
    }

    pub fn is_null(&self) -> bool {
        unsafe { sqlite3_value_type(self.handle.0.as_ptr()) == SQLITE_NULL }
    }
}

impl Drop for ValueHandle {
    fn drop(&mut self) {
        unsafe {
            sqlite3_value_free(self.0.as_ptr());
        }
    }
}
