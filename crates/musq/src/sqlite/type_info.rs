use std::{
    fmt::{self, Display, Formatter},
    result::Result as StdResult,
    str::FromStr,
};

use libsqlite3_sys::{SQLITE_BLOB, SQLITE_FLOAT, SQLITE_INTEGER, SQLITE_NULL, SQLITE_TEXT};

/// Data types supported by SQLite.
///
/// **Note:** This enum is marked `#[non_exhaustive]`; additional variants
/// may be added in the future. Avoid exhaustive matching.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum SqliteDataType {
    /// NULL value.
    Null,
    /// Integer value.
    Int,
    /// Floating-point value.
    Float,
    /// Text value.
    Text,
    /// Blob value.
    Blob,

    /// Values that follow SQLite's `NUMERIC` affinity.
    Numeric,

    // non-standard extensions
    /// Boolean value.
    Bool,
    /// 64-bit integer value.
    Int64,
    /// Date value.
    Date,
    /// Time value.
    Time,
    /// Datetime value.
    Datetime,
}

impl Display for SqliteDataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

impl SqliteDataType {
    /// Returns `true` if this is the NULL type.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Return the canonical SQLite type name.
    pub fn name(&self) -> &str {
        match self {
            Self::Null => "NULL",
            Self::Text => "TEXT",
            Self::Float => "REAL",
            Self::Blob => "BLOB",
            Self::Int | Self::Int64 => "INTEGER",
            Self::Numeric => "NUMERIC",

            // non-standard extensions
            Self::Bool => "BOOLEAN",
            Self::Date => "DATE",
            Self::Time => "TIME",
            Self::Datetime => "DATETIME",
        }
    }

    /// Convert a SQLite type code into a data type.
    pub(crate) fn from_code(code: i32) -> Option<Self> {
        match code {
            SQLITE_INTEGER => Some(Self::Int),
            SQLITE_FLOAT => Some(Self::Float),
            SQLITE_BLOB => Some(Self::Blob),
            SQLITE_NULL => Some(Self::Null),
            SQLITE_TEXT => Some(Self::Text),

            // https://sqlite.org/c3ref/c_blob.html
            _ => None,
        }
    }
}

// note: this implementation is particularly important as this is how the macros determine
//       what Rust type maps to what *declared* SQL type
// <https://www.sqlite.org/datatype3.html#affname>
impl FromStr for SqliteDataType {
    type Err = crate::Error;

    fn from_str(s: &str) -> StdResult<Self, Self::Err> {
        let original = s.to_owned();
        let s = original.to_ascii_lowercase();
        Ok(match &*s {
            "int4" => Self::Int,
            "int8" => Self::Int64,
            "boolean" | "bool" => Self::Bool,

            "date" => Self::Date,
            "time" => Self::Time,
            "datetime" | "timestamp" => Self::Datetime,

            _ if s.contains("int") => Self::Int64,

            _ if s.contains("char") || s.contains("clob") || s.contains("text") => Self::Text,

            _ if s.contains("blob") => Self::Blob,

            _ if s.contains("real") || s.contains("floa") || s.contains("doub") => Self::Float,

            _ if s.contains("num") || s.contains("dec") => Self::Numeric,

            _ => {
                return Err(crate::Error::TypeNotFound {
                    type_name: original,
                });
            }
        })
    }
}

/// Unit tests for SQLite type parsing.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_from_str() -> crate::Result<()> {
        assert_eq!(SqliteDataType::Int, "INT4".parse()?);

        assert_eq!(SqliteDataType::Int64, "INT".parse()?);
        assert_eq!(SqliteDataType::Int64, "INTEGER".parse()?);
        assert_eq!(SqliteDataType::Int64, "INTBIG".parse()?);
        assert_eq!(SqliteDataType::Int64, "MEDIUMINT".parse()?);

        assert_eq!(SqliteDataType::Int64, "BIGINT".parse()?);
        assert_eq!(SqliteDataType::Int64, "UNSIGNED BIG INT".parse()?);
        assert_eq!(SqliteDataType::Int64, "INT8".parse()?);

        assert_eq!(SqliteDataType::Text, "CHARACTER(20)".parse()?);
        assert_eq!(SqliteDataType::Text, "NCHAR(55)".parse()?);
        assert_eq!(SqliteDataType::Text, "TEXT".parse()?);
        assert_eq!(SqliteDataType::Text, "CLOB".parse()?);

        assert_eq!(SqliteDataType::Blob, "BLOB".parse()?);

        assert_eq!(SqliteDataType::Float, "REAL".parse()?);
        assert_eq!(SqliteDataType::Float, "FLOAT".parse()?);
        assert_eq!(SqliteDataType::Float, "DOUBLE PRECISION".parse()?);

        assert_eq!(SqliteDataType::Numeric, "NUMERIC".parse()?);
        assert_eq!(SqliteDataType::Numeric, "DECIMAL(10,5)".parse()?);

        assert_eq!(SqliteDataType::Bool, "BOOLEAN".parse()?);
        assert_eq!(SqliteDataType::Bool, "BOOL".parse()?);

        assert_eq!(SqliteDataType::Datetime, "DATETIME".parse()?);
        assert_eq!(SqliteDataType::Time, "TIME".parse()?);
        assert_eq!(SqliteDataType::Date, "DATE".parse()?);

        Ok(())
    }

    #[test]
    fn test_unknown_type_from_str() {
        match "UNKNOWN".parse::<SqliteDataType>() {
            Err(crate::Error::TypeNotFound { type_name }) => {
                assert_eq!(type_name, "UNKNOWN");
            }
            _ => panic!("expected TypeNotFound error"),
        }
    }

    #[test]
    fn test_from_code_unknown() {
        assert!(SqliteDataType::from_code(9999).is_none());
    }
}
