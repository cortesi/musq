use std::result::Result as StdResult;

use crate::{
    Result,
    decode::Decode,
    error::DecodeError,
    sqlite::{statement::StatementHandle, type_info::SqliteDataType},
};

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
        /// String value.
        value: String,
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
    pub fn int(&self) -> StdResult<i32, DecodeError> {
        match self {
            Self::Integer { value, .. } => Ok(i32::try_from(*value)?),
            Self::Null { .. } => Err(DecodeError::Conversion("unexpected NULL".into())),
            _ => Err(DecodeError::Conversion(
                "not an integer or out of range".into(),
            )),
        }
    }

    /// Returns the value as `i64` if it is stored as an integer.
    ///
    /// Fails with [`DecodeError::Conversion`] if the value is not an integer.
    pub fn int64(&self) -> StdResult<i64, DecodeError> {
        match self {
            Self::Integer { value, .. } => Ok(*value),
            Self::Null { .. } => Err(DecodeError::Conversion("unexpected NULL".into())),
            _ => Err(DecodeError::Conversion("not an integer".into())),
        }
    }

    /// Returns the value as `f64` if it is numeric.
    ///
    /// Integer values are automatically widened to `f64`. Any other variant
    /// results in a [`DecodeError::Conversion`].
    pub fn double(&self) -> StdResult<f64, DecodeError> {
        match self {
            Self::Double { value, .. } => Ok(*value),
            Self::Integer { value, .. } => Ok(*value as f64),
            Self::Null { .. } => Err(DecodeError::Conversion("unexpected NULL".into())),
            _ => Err(DecodeError::Conversion("not a double".into())),
        }
    }

    /// Returns the raw bytes contained in [`Value::Blob`] or [`Value::Text`].
    ///
    /// For other variants an empty slice is returned.
    pub fn blob(&self) -> StdResult<&[u8], DecodeError> {
        match self {
            Self::Blob { value, .. } => Ok(value.as_slice()),
            Self::Text { value, .. } => Ok(value.as_bytes()),
            Self::Null { .. } => Err(DecodeError::Conversion("unexpected NULL".into())),
            _ => Err(DecodeError::Conversion("not blob".into())),
        }
    }

    /// Interprets the value as UTF‑8 encoded text and returns it.
    ///
    /// Returns an error if the value is not [`Value::Text`].
    pub fn text(&self) -> StdResult<&str, DecodeError> {
        match self {
            Self::Text { value, .. } => Ok(value.as_str()),
            Self::Null { .. } => Err(DecodeError::Conversion("unexpected NULL".into())),
            _ => Err(DecodeError::Conversion("not text".into())),
        }
    }

    /// Returns the [`SqliteDataType`] associated with this value.
    ///
    /// If no explicit type information was captured when the value was read,
    /// the variant's natural type is returned instead.
    pub fn type_info(&self) -> SqliteDataType {
        match self {
            Self::Null { type_info } => type_info.unwrap_or(SqliteDataType::Null),
            Self::Integer { type_info, .. } => type_info.unwrap_or(SqliteDataType::Int),
            Self::Double { type_info, .. } => type_info.unwrap_or(SqliteDataType::Float),
            Self::Text { type_info, .. } => type_info.unwrap_or(SqliteDataType::Text),
            Self::Blob { type_info, .. } => type_info.unwrap_or(SqliteDataType::Blob),
        }
    }

    /// Returns `true` if the value is [`Value::Null`].
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null { .. })
    }

    /// Bind this value to the parameter `i` of the given statement handle.
    ///
    /// The binding is performed according to the underlying variant without
    /// altering the stored value.
    pub(crate) fn bind(&self, handle: &StatementHandle, i: usize) -> Result<()> {
        match self {
            Self::Text { value, .. } => handle.bind_text(i, value.as_str())?,
            Self::Blob { value, .. } => handle.bind_blob(i, value.as_slice())?,
            Self::Integer { value, .. } => handle.bind_int64(i, *value)?,
            Self::Double { value, .. } => handle.bind_double(i, *value)?,
            Self::Null { .. } => handle.bind_null(i)?,
        }

        Ok(())
    }
}

impl<'r> Decode<'r> for Value {
    fn decode(value: &'r Value) -> StdResult<Self, DecodeError> {
        Ok(value.clone())
    }
}
