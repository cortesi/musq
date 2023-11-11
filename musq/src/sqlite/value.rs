use std::{borrow::Cow, ptr::NonNull, slice::from_raw_parts, str::from_utf8, sync::Arc};

use libsqlite3_sys::{
    sqlite3_value, sqlite3_value_blob, sqlite3_value_bytes, sqlite3_value_double,
    sqlite3_value_dup, sqlite3_value_free, sqlite3_value_int, sqlite3_value_int64,
    sqlite3_value_type, SQLITE_NULL,
};

use crate::{error::BoxDynError, sqlite::type_info::SqliteDataType};

enum SqliteValueData<'r> {
    Value(&'r Value),
}

pub struct ValueRef<'r>(SqliteValueData<'r>);

impl<'r> ValueRef<'r> {
    pub fn value(value: &'r Value) -> Self {
        Self(SqliteValueData::Value(value))
    }

    pub fn int(&self) -> i32 {
        match self.0 {
            SqliteValueData::Value(v) => v.int(),
        }
    }

    pub fn int64(&self) -> i64 {
        match self.0 {
            SqliteValueData::Value(v) => v.int64(),
        }
    }

    pub fn double(&self) -> f64 {
        match self.0 {
            SqliteValueData::Value(v) => v.double(),
        }
    }

    pub fn blob(&self) -> &'r [u8] {
        match self.0 {
            SqliteValueData::Value(v) => v.blob(),
        }
    }

    pub fn text(&self) -> Result<&'r str, BoxDynError> {
        match self.0 {
            SqliteValueData::Value(v) => v.text(),
        }
    }

    pub fn to_owned(&self) -> Value {
        match self.0 {
            SqliteValueData::Value(v) => v.clone(),
        }
    }

    pub fn type_info(&self) -> Cow<'_, SqliteDataType> {
        match self.0 {
            SqliteValueData::Value(v) => v.type_info(),
        }
    }

    pub fn is_null(&self) -> bool {
        match self.0 {
            SqliteValueData::Value(v) => v.is_null(),
        }
    }
}

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
            handle: Arc::new(ValueHandle(NonNull::new_unchecked(sqlite3_value_dup(
                value,
            )))),
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

    fn int(&self) -> i32 {
        unsafe { sqlite3_value_int(self.handle.0.as_ptr()) }
    }

    fn int64(&self) -> i64 {
        unsafe { sqlite3_value_int64(self.handle.0.as_ptr()) }
    }

    fn double(&self) -> f64 {
        unsafe { sqlite3_value_double(self.handle.0.as_ptr()) }
    }

    fn blob(&self) -> &[u8] {
        let len = unsafe { sqlite3_value_bytes(self.handle.0.as_ptr()) } as usize;

        if len == 0 {
            // empty blobs are NULL so just return an empty slice
            return &[];
        }

        let ptr = unsafe { sqlite3_value_blob(self.handle.0.as_ptr()) } as *const u8;
        debug_assert!(!ptr.is_null());

        unsafe { from_raw_parts(ptr, len) }
    }

    fn text(&self) -> Result<&str, BoxDynError> {
        Ok(from_utf8(self.blob())?)
    }

    pub fn as_ref(&self) -> ValueRef<'_> {
        ValueRef::value(self)
    }

    pub fn type_info(&self) -> Cow<'_, SqliteDataType> {
        self.type_info_opt()
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed(&self.type_info))
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
