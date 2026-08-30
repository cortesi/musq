use std::collections::{HashMap, HashSet};

use crate::{
    Arguments, Conditions, Error, Result, Row,
    encode::Encode,
    executor::Execute,
    from_row::FromRow,
    query::{Map, Query},
};

#[derive(Default)]
/// Incrementally build a SQL query with bound parameters.
pub struct QueryBuilder {
    /// Accumulated SQL string.
    pub(crate) sql: String,
    /// Bound arguments.
    pub(crate) arguments: Arguments,
    /// Whether the query is tainted with raw SQL.
    pub(crate) tainted: bool,
}

impl QueryBuilder {
    /// Create a new, empty query builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder from existing parts.
    pub(crate) fn from_parts(sql: String, arguments: Arguments, tainted: bool) -> Self {
        Self {
            sql,
            arguments,
            tainted,
        }
    }

    /// Append raw SQL to the query.
    pub fn push_sql(&mut self, sql: &str) {
        self.sql.push_str(sql);
    }

    /// Append raw SQL and mark the query as tainted.
    pub fn push_raw(&mut self, raw: &str) {
        self.sql.push_str(raw);
        self.tainted = true;
    }

    /// Add a positional bind parameter and append the placeholder.
    pub fn push_bind<T: Encode>(&mut self, value: &T) -> Result<()> {
        self.arguments.add(value)?;
        self.sql.push('?');
        Ok(())
    }

    /// Add a named bind parameter and append the placeholder.
    pub fn push_bind_named<T: Encode>(&mut self, name: &str, value: &T) -> Result<()> {
        let name = normalize_bind_name(name)?;
        self.arguments.add_named(name, value)?;
        self.sql.push(':');
        self.sql.push_str(name);
        Ok(())
    }

    /// Append a comma-separated list of bound values.
    pub fn push_values<I, T>(&mut self, iter: I) -> Result<()>
    where
        I: IntoIterator<Item = T>,
        T: Encode,
    {
        let mut first = true;
        for v in iter {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            self.sql.push('?');
            self.arguments.add(&v)?;
        }
        Ok(())
    }

    /// Append a comma-separated list of quoted identifiers.
    pub fn push_idents<I>(&mut self, iter: I) -> Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut first = true;
        for ident in iter {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            self.sql.push_str(&crate::quote_identifier(ident.as_ref()));
        }
        if first {
            return Err(crate::Error::Query("empty idents".into()));
        }
        Ok(())
    }

    /// Append an INSERT column/value list from provided values.
    pub fn push_insert(&mut self, values: &crate::Values) -> Result<()> {
        if values.is_empty() {
            return Err(crate::Error::Query("empty values".into()));
        }
        self.sql.push('(');
        let mut first = true;
        for key in values.keys() {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            self.sql.push_str(&crate::quote_identifier(key));
        }
        self.sql.push_str(") VALUES (");
        first = true;
        for val in values.values() {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            match val {
                crate::ValuesEntry::Value(v) => {
                    self.sql.push('?');
                    self.arguments.values.push(v.clone());
                }
                crate::ValuesEntry::Expr(expr) => {
                    self.push_fragment(
                        expr.sql.clone(),
                        expr.arguments.clone(),
                        expr.tainted,
                        true,
                    )?;
                }
            }
        }
        self.sql.push(')');
        Ok(())
    }

    /// Append a SET clause from provided values.
    pub fn push_set(&mut self, values: &crate::Values) -> Result<()> {
        if values.is_empty() {
            return Err(crate::Error::Query("empty values".into()));
        }
        let mut first = true;
        for (k, entry) in values.iter() {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            self.sql.push_str(&crate::quote_identifier(k));
            match entry {
                crate::ValuesEntry::Value(v) => {
                    self.sql.push_str(" = ?");
                    self.arguments.values.push(v.clone());
                }
                crate::ValuesEntry::Expr(expr) => {
                    self.sql.push_str(" = ");
                    self.push_fragment(
                        expr.sql.clone(),
                        expr.arguments.clone(),
                        expr.tainted,
                        true,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Append a WHERE clause from provided values.
    pub fn push_where(&mut self, values: &crate::Values) -> Result<()> {
        if values.is_empty() {
            self.sql.push_str("1=1");
            return Ok(());
        }
        let mut first = true;
        for (k, entry) in values.iter() {
            if !first {
                self.sql.push_str(" AND ");
            }
            first = false;
            self.sql.push_str(&crate::quote_identifier(k));
            match entry {
                crate::ValuesEntry::Value(v) => match v {
                    crate::Value::Null { .. } => self.sql.push_str(" IS NULL"),
                    _ => {
                        self.sql.push_str(" = ?");
                        self.arguments.values.push(v.clone());
                    }
                },
                crate::ValuesEntry::Expr(expr) => {
                    self.sql.push_str(" = ");
                    self.push_fragment(
                        expr.sql.clone(),
                        expr.arguments.clone(),
                        expr.tainted,
                        true,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Append an UPSERT update clause, excluding the named columns.
    pub fn push_upsert(&mut self, values: &crate::Values, exclude: &[&str]) -> Result<()> {
        if values.is_empty() {
            return Err(crate::Error::Query("empty values".into()));
        }

        let exclude: HashSet<&str> = exclude.iter().copied().collect();

        if values.keys().all(|k| exclude.contains(k.as_str())) {
            return Err(crate::Error::Query("empty values".into()));
        }

        let mut first = true;
        for key in values.keys() {
            if exclude.contains(key.as_str()) {
                continue;
            }
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            let ident = crate::quote_identifier(key);
            self.sql.push_str(&ident);
            self.sql.push_str(" = excluded.");
            self.sql.push_str(&ident);
        }

        if first {
            return Err(crate::Error::Query("empty values".into()));
        }

        Ok(())
    }

    /// Appends another [`Query`] to this builder.
    ///
    /// The SQL of the provided query is appended to this builder with a single
    /// space in between if needed. All arguments from the other query are
    /// merged and indices for named parameters are re-based to ensure they
    /// refer to the correct values.
    ///
    /// This method panics if the appended query contains numeric positional
    /// placeholders such as `?1` or numeric `$1`. Use
    /// [`QueryBuilder::try_push_query`] to handle unsupported composition as
    /// an error.
    pub fn push_query(&mut self, query: Query) {
        self.try_push_query(query)
            .expect("failed to append query fragment")
    }

    /// Attempt to append another [`Query`] to this builder.
    ///
    /// Numeric positional placeholders such as `?1` and numeric `$1` are
    /// rejected in appended fragments because their absolute SQLite indices
    /// cannot be safely rebased by the current composition machinery.
    pub fn try_push_query(&mut self, query: Query) -> Result<()> {
        if !query.sql().is_empty() {
            let needs_space = !self.sql.is_empty();
            let tainted = query.tainted;
            if !self.sql.is_empty() {
                self.sql.push(' ');
            }
            let sql = query.sql;
            if let Err(err) =
                self.push_fragment(sql, query.arguments.unwrap_or_default(), tainted, false)
            {
                if needs_space {
                    self.sql.pop();
                }
                return Err(err);
            }
        }
        Ok(())
    }

    /// Append a typed collection as one `WHERE` clause.
    pub fn push_conditions(&mut self, conditions: Conditions) -> Result<()> {
        if let Some(query) = conditions.into_query()? {
            self.try_push_query(query)?;
        }
        Ok(())
    }

    /// Append a SQL fragment with arguments, rebasing/renaming named parameters
    /// as needed.
    fn push_fragment(
        &mut self,
        mut sql: String,
        other_args: Arguments,
        tainted: bool,
        namespace_named: bool,
    ) -> Result<()> {
        reject_numeric_parameters(&sql)?;

        let base_index = self.arguments.values.len();
        self.arguments.values.extend(other_args.values);

        if !other_args.named.is_empty() {
            let mut used_names: HashSet<String> = self.arguments.named.keys().cloned().collect();
            if !namespace_named {
                used_names.extend(other_args.named.keys().cloned());
            }

            let mut renames: HashMap<String, String> = HashMap::new();
            for (name, index) in other_args.named {
                let name = if namespace_named {
                    let base = format!("__musq_expr_{name}");
                    let new_name = disambiguate_name(&base, &mut used_names);
                    renames.insert(name.clone(), new_name.clone());
                    new_name
                } else if self.arguments.named.contains_key(&name) {
                    let new_name = disambiguate_name(&name, &mut used_names);
                    renames.insert(name.clone(), new_name.clone());
                    new_name
                } else {
                    used_names.insert(name.clone());
                    name
                };

                self.arguments.named.insert(name, base_index + index);
            }

            if !renames.is_empty() {
                sql = rewrite_named_parameters(&sql, &renames);
            }
        }

        self.sql.push_str(&sql);
        self.tainted |= tainted;
        Ok(())
    }

    /// Finalize the builder into a [`Query`].
    pub fn build(self) -> Query {
        Query {
            sql: self.sql,
            arguments: Some(self.arguments),
            tainted: self.tainted,
        }
    }

    /// Finalize this builder as a query mapped through [`FromRow`].
    pub fn build_query_as<O>(self) -> Map<impl FnMut(Row) -> Result<O> + Send>
    where
        O: Send + Unpin + for<'row> FromRow<'row>,
    {
        self.build().try_map(|row| O::from_row("", &row))
    }
}

/// Normalize a named bind argument into the bare SQLite parameter name.
fn normalize_bind_name(name: &str) -> Result<&str> {
    let name = name.trim_start_matches([':', '@', '$', '?']);
    if name.is_empty() {
        return Err(Error::Query("empty named bind parameter".into()));
    }
    Ok(name)
}

/// Reject numeric placeholders in fragments that are being composed.
fn reject_numeric_parameters(sql: &str) -> Result<()> {
    if contains_numeric_parameter(sql) {
        return Err(Error::Query(
            "numeric SQL parameters are not supported in composed query fragments".into(),
        ));
    }
    Ok(())
}

/// Returns a unique named-parameter identifier by appending a numeric suffix.
fn disambiguate_name(name: &str, used_names: &mut HashSet<String>) -> String {
    let mut suffix = 1_usize;
    loop {
        let candidate = format!("{name}_{suffix}");
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Returns `true` if this byte is treated as an identifier character for the
/// purposes of rewriting named parameters.
fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// SQL span outside strings and comments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlToken<'a> {
    /// Literal text, including quoted strings and comments.
    Text(&'a str),
    /// Named placeholder such as `:name`, `@name`, or `$name`.
    Placeholder {
        /// `$`, `:`, or `@`.
        prefix: char,
        /// Name without the prefix.
        name: &'a str,
    },
    /// Numeric placeholder such as `?1` or `$2`.
    Numeric {
        /// `?` or `$`.
        prefix: char,
        /// Decimal digits after the prefix.
        digits: &'a str,
    },
}

/// Scanner state for strings, quoted identifiers, and comments.
#[derive(Clone, Copy)]
enum ScanState {
    /// Outside quotes and comments.
    Normal,
    /// Inside a single-quoted string.
    SingleQuote,
    /// Inside a double-quoted identifier or string.
    DoubleQuote,
    /// Inside a `--` line comment.
    LineComment,
    /// Inside a `/* */` block comment.
    BlockComment,
}

/// Walk `sql` and visit tokens outside strings and comments.
fn scan_sql(sql: &str, mut visit: impl FnMut(SqlToken<'_>)) {
    let bytes = sql.as_bytes();
    let mut i = 0;
    let mut text_start = 0;
    let mut state = ScanState::Normal;

    while i < bytes.len() {
        match state {
            ScanState::Normal => match bytes[i] {
                b'\'' => {
                    i += 1;
                    state = ScanState::SingleQuote;
                }
                b'"' => {
                    i += 1;
                    state = ScanState::DoubleQuote;
                }
                b'-' if bytes.get(i + 1) == Some(&b'-') => {
                    i += 2;
                    state = ScanState::LineComment;
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    i += 2;
                    state = ScanState::BlockComment;
                }
                b'?' if bytes.get(i + 1).is_some_and(u8::is_ascii_digit) => {
                    let start = i;
                    i += 1;
                    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                        i += 1;
                    }
                    if start > text_start {
                        visit(SqlToken::Text(&sql[text_start..start]));
                    }
                    visit(SqlToken::Numeric {
                        prefix: '?',
                        digits: &sql[start + 1..i],
                    });
                    text_start = i;
                }
                b'$' | b':' | b'@' => {
                    let prefix = bytes[i] as char;
                    let name_start = i + 1;
                    let mut end = name_start;
                    while end < bytes.len() && is_ident_char(bytes[end]) {
                        end += 1;
                    }
                    if end > name_start {
                        let name = &sql[name_start..end];
                        let all_digits = name.as_bytes().iter().all(u8::is_ascii_digit);
                        if i > text_start {
                            visit(SqlToken::Text(&sql[text_start..i]));
                        }
                        if prefix == '$' && all_digits {
                            visit(SqlToken::Numeric {
                                prefix: '$',
                                digits: name,
                            });
                        } else {
                            visit(SqlToken::Placeholder { prefix, name });
                        }
                        i = end;
                        text_start = i;
                    } else {
                        i += 1;
                    }
                }
                _ => i += 1,
            },
            ScanState::SingleQuote => {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                    } else {
                        i += 1;
                        state = ScanState::Normal;
                    }
                } else {
                    i += 1;
                }
            }
            ScanState::DoubleQuote => {
                if bytes[i] == b'"' {
                    if bytes.get(i + 1) == Some(&b'"') {
                        i += 2;
                    } else {
                        i += 1;
                        state = ScanState::Normal;
                    }
                } else {
                    i += 1;
                }
            }
            ScanState::LineComment => {
                if bytes[i] == b'\n' {
                    state = ScanState::Normal;
                }
                i += 1;
            }
            ScanState::BlockComment => {
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    i += 2;
                    state = ScanState::Normal;
                } else {
                    i += 1;
                }
            }
        }
    }
    if bytes.len() > text_start {
        visit(SqlToken::Text(&sql[text_start..]));
    }
}

/// Returns `true` if SQL contains `?NNN` or numeric `$NNN` placeholders outside
/// strings, quoted identifiers, and comments.
fn contains_numeric_parameter(sql: &str) -> bool {
    let mut found = false;
    scan_sql(sql, |token| {
        if matches!(token, SqlToken::Numeric { .. }) {
            found = true;
        }
    });
    found
}

/// Rewrites named parameters (e.g. `:name`, `@name`, `$name`) according to the
/// provided mapping, skipping string literals, quoted identifiers, and
/// comments.
fn rewrite_named_parameters(sql: &str, renames: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(sql.len());
    scan_sql(sql, |token| match token {
        SqlToken::Text(text) => out.push_str(text),
        SqlToken::Numeric { prefix, digits } => {
            out.push(prefix);
            out.push_str(digits);
        }
        SqlToken::Placeholder { prefix, name } => {
            out.push(prefix);
            if let Some(new_name) = renames.get(name) {
                out.push_str(new_name);
            } else {
                out.push_str(name);
            }
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{contains_numeric_parameter, rewrite_named_parameters, scan_sql};

    #[derive(Debug, PartialEq, Eq)]
    enum OwnedToken {
        Text(String),
        Placeholder { prefix: char, name: String },
        Numeric { prefix: char, digits: String },
    }

    fn collect(sql: &str) -> Vec<OwnedToken> {
        let mut out = Vec::new();
        scan_sql(sql, |token| {
            out.push(match token {
                super::SqlToken::Text(text) => OwnedToken::Text(text.to_owned()),
                super::SqlToken::Placeholder { prefix, name } => OwnedToken::Placeholder {
                    prefix,
                    name: name.to_owned(),
                },
                super::SqlToken::Numeric { prefix, digits } => OwnedToken::Numeric {
                    prefix,
                    digits: digits.to_owned(),
                },
            });
        });
        out
    }

    #[test]
    fn scan_sql_table() {
        assert_eq!(collect("SELECT 1"), [OwnedToken::Text("SELECT 1".into())]);
        assert_eq!(
            collect("WHERE id = :id"),
            [
                OwnedToken::Text("WHERE id = ".into()),
                OwnedToken::Placeholder {
                    prefix: ':',
                    name: "id".into()
                },
            ]
        );
        assert_eq!(
            collect("a $2 b"),
            [
                OwnedToken::Text("a ".into()),
                OwnedToken::Numeric {
                    prefix: '$',
                    digits: "2".into()
                },
                OwnedToken::Text(" b".into()),
            ]
        );
        assert_eq!(
            collect("a ?1 b"),
            [
                OwnedToken::Text("a ".into()),
                OwnedToken::Numeric {
                    prefix: '?',
                    digits: "1".into()
                },
                OwnedToken::Text(" b".into()),
            ]
        );
        assert_eq!(collect("':id'"), [OwnedToken::Text("':id'".into())]);
        assert_eq!(
            collect("-- :id\n:x"),
            [
                OwnedToken::Text("-- :id\n".into()),
                OwnedToken::Placeholder {
                    prefix: ':',
                    name: "x".into()
                },
            ]
        );
        assert_eq!(
            collect("$2foo"),
            [OwnedToken::Placeholder {
                prefix: '$',
                name: "2foo".into()
            }]
        );
        assert!(contains_numeric_parameter("SELECT ?1"));
        assert!(!contains_numeric_parameter("SELECT :id"));
        let mut renames = HashMap::new();
        renames.insert("id".into(), "id_0".into());
        assert_eq!(
            rewrite_named_parameters("WHERE x = :id", &renames),
            "WHERE x = :id_0"
        );
        assert_eq!(
            rewrite_named_parameters("WHERE x = ':id'", &renames),
            "WHERE x = ':id'"
        );
    }
}
