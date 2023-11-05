use std::fmt::{self, Display, Formatter};
use std::os::raw::c_int;
use std::str::FromStr;

use libsqlite3_sys::{SQLITE_BLOB, SQLITE_FLOAT, SQLITE_INTEGER, SQLITE_NULL, SQLITE_TEXT};

use crate::error::BoxDynError;

/// Data types supported by SQLite.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SqliteDataType {
    Null,
    Int,
    Float,
    Text,
    Blob,

    // TODO: Support NUMERIC
    #[allow(dead_code)]
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

    pub(crate) fn from_code(code: c_int) -> Self {
        match code {
            SQLITE_INTEGER => SqliteDataType::Int,
            SQLITE_FLOAT => SqliteDataType::Float,
            SQLITE_BLOB => SqliteDataType::Blob,
            SQLITE_NULL => SqliteDataType::Null,
            SQLITE_TEXT => SqliteDataType::Text,

            // https://sqlite.org/c3ref/c_blob.html
            _ => panic!("unknown data type code {}", code),
        }
    }
}

// note: this implementation is particularly important as this is how the macros determine
//       what Rust type maps to what *declared* SQL type
// <https://www.sqlite.org/datatype3.html#affname>
impl FromStr for SqliteDataType {
    type Err = BoxDynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_ascii_lowercase();
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

            _ => {
                return Err(format!("unknown type: `{}`", s).into());
            }
        })
    }
}

#[test]
fn test_data_type_from_str() -> Result<(), BoxDynError> {
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

    assert_eq!(SqliteDataType::Bool, "BOOLEAN".parse()?);
    assert_eq!(SqliteDataType::Bool, "BOOL".parse()?);

    assert_eq!(SqliteDataType::Datetime, "DATETIME".parse()?);
    assert_eq!(SqliteDataType::Time, "TIME".parse()?);
    assert_eq!(SqliteDataType::Date, "DATE".parse()?);

    Ok(())
}
