use std::fmt::{self, Display, Formatter};

use libsqlite3_sys::{SQLITE_BLOB, SQLITE_FLOAT, SQLITE_INTEGER, SQLITE_NULL, SQLITE_TEXT};

/// Data types supported by SQLite.
///
/// **Note:** This enum is marked `#[non_exhaustive]`; additional variants
/// may be added in the future. Avoid exhaustive matching.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
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

    /// Parse a declared SQL type name using SQLite affinity rules.
    ///
    /// See <https://www.sqlite.org/datatype3.html#affname>.
    #[allow(
        clippy::should_implement_trait,
        reason = "this parser returns Option, not Result"
    )]
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.to_ascii_lowercase();
        Some(match &*s {
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

            _ => return None,
        })
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

/// Unit tests for SQLite type parsing.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_from_str() {
        assert_eq!(SqliteDataType::from_str("INT4"), Some(SqliteDataType::Int));

        assert_eq!(SqliteDataType::from_str("INT"), Some(SqliteDataType::Int64));
        assert_eq!(
            SqliteDataType::from_str("INTEGER"),
            Some(SqliteDataType::Int64)
        );
        assert_eq!(
            SqliteDataType::from_str("INTBIG"),
            Some(SqliteDataType::Int64)
        );
        assert_eq!(
            SqliteDataType::from_str("MEDIUMINT"),
            Some(SqliteDataType::Int64)
        );

        assert_eq!(
            SqliteDataType::from_str("BIGINT"),
            Some(SqliteDataType::Int64)
        );
        assert_eq!(
            SqliteDataType::from_str("UNSIGNED BIG INT"),
            Some(SqliteDataType::Int64)
        );
        assert_eq!(
            SqliteDataType::from_str("INT8"),
            Some(SqliteDataType::Int64)
        );

        assert_eq!(
            SqliteDataType::from_str("CHARACTER(20)"),
            Some(SqliteDataType::Text)
        );
        assert_eq!(
            SqliteDataType::from_str("NCHAR(55)"),
            Some(SqliteDataType::Text)
        );
        assert_eq!(SqliteDataType::from_str("TEXT"), Some(SqliteDataType::Text));
        assert_eq!(SqliteDataType::from_str("CLOB"), Some(SqliteDataType::Text));

        assert_eq!(SqliteDataType::from_str("BLOB"), Some(SqliteDataType::Blob));

        assert_eq!(
            SqliteDataType::from_str("REAL"),
            Some(SqliteDataType::Float)
        );
        assert_eq!(
            SqliteDataType::from_str("FLOAT"),
            Some(SqliteDataType::Float)
        );
        assert_eq!(
            SqliteDataType::from_str("DOUBLE PRECISION"),
            Some(SqliteDataType::Float)
        );

        assert_eq!(
            SqliteDataType::from_str("NUMERIC"),
            Some(SqliteDataType::Numeric)
        );
        assert_eq!(
            SqliteDataType::from_str("DECIMAL(10,5)"),
            Some(SqliteDataType::Numeric)
        );

        assert_eq!(
            SqliteDataType::from_str("BOOLEAN"),
            Some(SqliteDataType::Bool)
        );
        assert_eq!(SqliteDataType::from_str("BOOL"), Some(SqliteDataType::Bool));

        assert_eq!(
            SqliteDataType::from_str("DATETIME"),
            Some(SqliteDataType::Datetime)
        );
        assert_eq!(SqliteDataType::from_str("TIME"), Some(SqliteDataType::Time));
        assert_eq!(SqliteDataType::from_str("DATE"), Some(SqliteDataType::Date));
    }

    #[test]
    fn test_unknown_type_from_str() {
        assert_eq!(SqliteDataType::from_str("UNKNOWN"), None);
    }

    #[test]
    fn test_from_code_unknown() {
        assert!(SqliteDataType::from_code(9999).is_none());
    }
}
