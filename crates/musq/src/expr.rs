//! Helpers for building safe SQL expressions for use with `Values`.
//!
//! Expressions are SQL fragments that can be embedded into `{set:...}`, `{insert:...}`, and
//! `{where:...}` placeholders. Unlike regular bound values, expressions may expand to arbitrary SQL
//! (optionally containing their own bind parameters).
//!
//! Use curated helpers (like [`now_rfc3339_utc`]) whenever possible. If you must embed ad-hoc SQL,
//! use [`raw`], which will taint the resulting query.

use either::Either;

use crate::{Arguments, Query, Value, error::EncodeError};

/// A SQL expression fragment with optional bound parameters.
///
/// This type is intentionally not `Encode`; it is meant to be embedded into composed SQL, not
/// bound as a single SQLite value.
#[derive(Debug, Clone)]
pub struct Expr {
    /// SQL for the expression fragment.
    pub(crate) sql: String,
    /// Arguments referenced by the expression fragment.
    pub(crate) arguments: Arguments,
    /// Whether the expression is tainted with raw SQL.
    pub(crate) tainted: bool,
}

impl Expr {
    /// Returns the SQL text for this expression.
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// Returns `true` if this expression includes raw SQL and should be treated as tainted.
    pub fn tainted(&self) -> bool {
        self.tainted
    }
}

impl From<Query> for Expr {
    fn from(query: Query) -> Self {
        let sql = match query.statement {
            Either::Left(sql) => sql,
            Either::Right(statement) => statement.sql,
        };
        let arguments = query.arguments.unwrap_or_default();
        Self {
            sql,
            arguments,
            tainted: query.tainted,
        }
    }
}

/// SQLite expression for the current UTC time, formatted as RFC3339.
///
/// This is intended to match hb's documented storage format for timestamps.
pub fn now_rfc3339_utc() -> Expr {
    Expr {
        sql: "STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')".to_string(),
        arguments: Arguments::default(),
        tainted: false,
    }
}

/// Wrap a JSON text value with SQLite's `jsonb(...)` function.
///
/// The JSON text is bound as a parameter; SQLite will store it using its internal JSONB encoding.
pub fn jsonb(json: &str) -> Expr {
    let mut arguments = Arguments::default();
    arguments.values.push(Value::Text {
        value: json.to_string().into(),
        type_info: None,
    });
    Expr {
        sql: "jsonb(?)".to_string(),
        arguments,
        tainted: false,
    }
}

/// Wrap a JSON text value with SQLite's `jsonb(...)` function.
///
/// Alias for [`jsonb`].
pub fn jsonb_text(json: &str) -> Expr {
    jsonb(json)
}

/// Serialize a value to JSON and wrap it with SQLite's `jsonb(...)` function.
pub fn jsonb_serde<T>(value: &T) -> crate::Result<Expr>
where
    T: serde::Serialize + ?Sized,
{
    let json = serde_json::to_string(value).map_err(|e| {
        crate::Error::Encode(EncodeError::Conversion(format!(
            "failed to encode value as JSON: {e}"
        )))
    })?;

    let mut arguments = Arguments::default();
    arguments.values.push(Value::Text {
        value: json.into(),
        type_info: None,
    });
    Ok(Expr {
        sql: "jsonb(?)".to_string(),
        arguments,
        tainted: false,
    })
}

/// Embed raw SQL as an expression and taint the resulting query.
pub fn raw(sql: &str) -> Expr {
    Expr {
        sql: sql.to_string(),
        arguments: Arguments::default(),
        tainted: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_expr_is_tainted() {
        assert!(raw("1").tainted());
    }

    #[test]
    fn now_expr_is_not_tainted() {
        assert!(!now_rfc3339_utc().tainted());
    }

    #[test]
    fn jsonb_serde_roundtrips() {
        #[derive(serde::Serialize)]
        struct Payload {
            a: i32,
        }

        let expr = jsonb_serde(&Payload { a: 1 }).unwrap();
        assert_eq!(expr.sql(), "jsonb(?)");
        assert!(!expr.tainted());
    }
}
