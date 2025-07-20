use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use libsqlite3_sys::{SQLITE_BLOB, SQLITE_FLOAT, SQLITE_INTEGER, SQLITE_NULL, SQLITE_TEXT};

/// Data types supported by SQLite.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SqliteDataType {
    Null,
    Int,
    Float,
    Text,
    Blob,

    /// Values that follow SQLite's `NUMERIC` affinity.
    Numeric,

    // non-standard extensions
    Bool,
    Int64,
    Date,
    Time,
    Datetime,
}

impl Display for SqliteDataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

impl SqliteDataType {
    pub fn is_null(&self) -> bool {
        matches!(self, SqliteDataType::Null)
    }

    pub fn name(&self) -> &str {
        match self {
            SqliteDataType::Null => "NULL",
            SqliteDataType::Text => "TEXT",
            SqliteDataType::Float => "REAL",
            SqliteDataType::Blob => "BLOB",
            SqliteDataType::Int | SqliteDataType::Int64 => "INTEGER",
            SqliteDataType::Numeric => "NUMERIC",

            // non-standard extensions
            SqliteDataType::Bool => "BOOLEAN",
            SqliteDataType::Date => "DATE",
            SqliteDataType::Time => "TIME",
            SqliteDataType::Datetime => "DATETIME",
        }
    }

    pub(crate) fn from_code(code: i32) -> Option<Self> {
        match code {
            SQLITE_INTEGER => Some(SqliteDataType::Int),
            SQLITE_FLOAT => Some(SqliteDataType::Float),
            SQLITE_BLOB => Some(SqliteDataType::Blob),
            SQLITE_NULL => Some(SqliteDataType::Null),
            SQLITE_TEXT => Some(SqliteDataType::Text),

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

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let original = s.to_owned();
        let s = original.to_ascii_lowercase();
        Ok(match &*s {
            "int4" => SqliteDataType::Int,
            "int8" => SqliteDataType::Int64,
            "boolean" | "bool" => SqliteDataType::Bool,

            "date" => SqliteDataType::Date,
            "time" => SqliteDataType::Time,
            "datetime" | "timestamp" => SqliteDataType::Datetime,

            _ if s.contains("int") => SqliteDataType::Int64,

            _ if s.contains("char") || s.contains("clob") || s.contains("text") => {
                SqliteDataType::Text
            }

            _ if s.contains("blob") => SqliteDataType::Blob,

            _ if s.contains("real") || s.contains("floa") || s.contains("doub") => {
                SqliteDataType::Float
            }

            _ if s.contains("num") || s.contains("dec") => SqliteDataType::Numeric,

            _ => {
                return Err(crate::Error::TypeNotFound {
                    type_name: original,
                });
            }
        })
    }
}

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
