use std::str::from_utf8;

use crate::{error::DecodeError, sqlite::type_info::SqliteDataType};

/// Owned representation of a SQLite value.
///
/// Each variant stores the underlying value along with an optional
/// [`SqliteDataType`] describing how SQLite declared the column.
/// When the type information is omitted the variant's natural type is used.
///
/// The variants closely mirror SQLite's dynamic typing system and allow a
/// single enum to capture every value that can be transferred between the
/// database and user code.
///
/// **Note:** This enum is marked `#[non_exhaustive]`; do not match on it
/// exhaustively as more variants may be added in the future.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Value {
    /// A NULL value. If the original column declared a specific type the
    /// information is stored in `type_info` for use during decoding.
    Null {
        /// Original declared type, if known.
        type_info: Option<SqliteDataType>,
    },
    /// A 64-bit signed integer.
    Integer {
        /// The raw integer value.
        value: i64,
        /// Original declared type, if known.
        type_info: Option<SqliteDataType>,
    },
    /// A double precision floating point number.
    Double {
        /// The raw floating point value.
        value: f64,
        /// Original declared type, if known.
        type_info: Option<SqliteDataType>,
    },
    /// A UTF-8 text value.
    Text {
        /// Bytes making up the textual value.
        value: Vec<u8>,
        /// Original declared type, if known.
        type_info: Option<SqliteDataType>,
    },
    /// An arbitrary blob of bytes.
    Blob {
        /// Bytes contained in the blob.
        value: Vec<u8>,
        /// Original declared type, if known.
        type_info: Option<SqliteDataType>,
    },
}

impl Value {
    /// Returns the value as `i32` if it is an integer and fits in the range.
    pub fn int(&self) -> std::result::Result<i32, DecodeError> {
        Ok(i32::try_from(self.int64()?)?)
    }

    /// Returns the value as `i64` if it is stored as an integer.
    ///
    /// Fails with [`DecodeError::Conversion`] if the value is not an integer.
    pub fn int64(&self) -> std::result::Result<i64, DecodeError> {
        match self {
            Value::Integer { value, .. } => Ok(*value),
            _ => Err(DecodeError::Conversion("not an integer".into())),
        }
    }

    /// Returns the value as `f64` if it is numeric.
    ///
    /// Integer values are automatically widened to `f64`. Any other variant
    /// results in a [`DecodeError::Conversion`].
    pub fn double(&self) -> std::result::Result<f64, DecodeError> {
        match self {
            Value::Double { value, .. } => Ok(*value),
            Value::Integer { value, .. } => Ok(*value as f64),
            _ => Err(DecodeError::Conversion("not a float".into())),
        }
    }

    /// Returns the raw bytes contained in [`Value::Blob`] or [`Value::Text`].
    ///
    /// For other variants an empty slice is returned.
    pub fn blob(&self) -> &[u8] {
        match self {
            Value::Blob { value, .. } | Value::Text { value, .. } => value.as_slice(),
            _ => &[],
        }
    }

    /// Interprets the value as UTF‑8 encoded text and returns it.
    ///
    /// Returns an error if the value is not [`Value::Text`] or contains invalid
    /// UTF‑8 bytes.
    pub fn text(&self) -> std::result::Result<&str, DecodeError> {
        match self {
            Value::Text { value, .. } => {
                from_utf8(value).map_err(|e| DecodeError::Conversion(e.to_string()))
            }
            _ => Err(DecodeError::Conversion("not text".into())),
        }
    }

    /// Returns the [`SqliteDataType`] associated with this value.
    ///
    /// If no explicit type information was captured when the value was read,
    /// the variant's natural type is returned instead.
    pub fn type_info(&self) -> SqliteDataType {
        match self {
            Value::Null { type_info } => type_info.unwrap_or(SqliteDataType::Null),
            Value::Integer { type_info, .. } => type_info.unwrap_or(SqliteDataType::Int),
            Value::Double { type_info, .. } => type_info.unwrap_or(SqliteDataType::Float),
            Value::Text { type_info, .. } => type_info.unwrap_or(SqliteDataType::Text),
            Value::Blob { type_info, .. } => type_info.unwrap_or(SqliteDataType::Blob),
        }
    }

    /// Returns `true` if the value is [`Value::Null`].
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null { .. })
    }
}
