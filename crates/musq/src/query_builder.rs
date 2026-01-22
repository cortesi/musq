use std::collections::{HashMap, HashSet};

use either::Either;

use crate::{Arguments, Result, encode::Encode, executor::Execute, query::Query};

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
        if first {
            return Err(crate::Error::Protocol("empty values".into()));
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
            return Err(crate::Error::Protocol("empty idents".into()));
        }
        Ok(())
    }

    /// Append an INSERT column/value list from provided values.
    pub fn push_insert(&mut self, values: &crate::Values) -> Result<()> {
        if values.is_empty() {
            return Err(crate::Error::Protocol("empty values".into()));
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
                    );
                }
            }
        }
        self.sql.push(')');
        Ok(())
    }

    /// Append a SET clause from provided values.
    pub fn push_set(&mut self, values: &crate::Values) -> Result<()> {
        if values.is_empty() {
            return Err(crate::Error::Protocol("empty values".into()));
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
                    );
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
                    );
                }
            }
        }
        Ok(())
    }

    /// Append an UPSERT update clause, excluding the named columns.
    pub fn push_upsert(&mut self, values: &crate::Values, exclude: &[&str]) -> Result<()> {
        if values.is_empty() {
            return Err(crate::Error::Protocol("empty values".into()));
        }

        let exclude: HashSet<&str> = exclude.iter().copied().collect();

        if values.keys().all(|k| exclude.contains(k.as_str())) {
            return Err(crate::Error::Protocol("empty values".into()));
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
            return Err(crate::Error::Protocol("empty values".into()));
        }

        Ok(())
    }

    /// Appends another [`Query`] to this builder.
    ///
    /// The SQL of the provided query is appended to this builder with a single
    /// space in between if needed. All arguments from the other query are
    /// merged and indices for named parameters are re-based to ensure they
    /// refer to the correct values.
    pub fn push_query(&mut self, query: Query) {
        if !query.sql().is_empty() {
            if !self.sql.is_empty() {
                self.sql.push(' ');
            }
            let sql = match query.statement {
                Either::Left(sql) => sql,
                Either::Right(statement) => statement.sql,
            };
            self.push_fragment(
                sql,
                query.arguments.unwrap_or_default(),
                query.tainted,
                false,
            );
        }
    }

    /// Append a SQL fragment with arguments, rebasing/renaming named parameters as needed.
    fn push_fragment(
        &mut self,
        mut sql: String,
        other_args: Arguments,
        tainted: bool,
        namespace_named: bool,
    ) {
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
    }

    /// Finalize the builder into a [`Query`].
    pub fn build(self) -> Query {
        Query {
            statement: Either::Left(self.sql),
            arguments: Some(self.arguments),
            tainted: self.tainted,
        }
    }
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

/// Rewrites named parameters (e.g. `:name`, `@name`, `$name`) according to the
/// provided mapping, skipping string literals, quoted identifiers, and comments.
fn rewrite_named_parameters(sql: &str, renames: &HashMap<String, String>) -> String {
    #[derive(Clone, Copy, Debug)]
    enum State {
        Normal,
        SingleQuote,
        DoubleQuote,
        LineComment,
        BlockComment,
    }

    let mut out = Vec::with_capacity(sql.len());
    let mut i = 0;
    let bytes = sql.as_bytes();
    let mut state = State::Normal;

    while i < bytes.len() {
        match state {
            State::Normal => match bytes[i] {
                b'\'' => {
                    out.push(bytes[i]);
                    i += 1;
                    state = State::SingleQuote;
                }
                b'"' => {
                    out.push(bytes[i]);
                    i += 1;
                    state = State::DoubleQuote;
                }
                b'-' if bytes.get(i + 1) == Some(&b'-') => {
                    out.extend_from_slice(b"--");
                    i += 2;
                    state = State::LineComment;
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    out.extend_from_slice(b"/*");
                    i += 2;
                    state = State::BlockComment;
                }
                b':' | b'@' | b'$' => {
                    let prefix = bytes[i];
                    let start = i + 1;
                    let mut end = start;
                    while end < bytes.len() && is_ident_char(bytes[end]) {
                        end += 1;
                    }

                    if end > start {
                        let name = &sql[start..end];
                        if let Some(new_name) = renames.get(name) {
                            out.push(prefix);
                            out.extend_from_slice(new_name.as_bytes());
                        } else {
                            out.extend_from_slice(&bytes[i..end]);
                        }
                        i = end;
                    } else {
                        out.push(prefix);
                        i += 1;
                    }
                }
                _ => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            State::SingleQuote => {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        out.extend_from_slice(b"''");
                        i += 2;
                    } else {
                        out.push(bytes[i]);
                        i += 1;
                        state = State::Normal;
                    }
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            State::DoubleQuote => {
                if bytes[i] == b'"' {
                    if bytes.get(i + 1) == Some(&b'"') {
                        out.extend_from_slice(br#""""#);
                        i += 2;
                    } else {
                        out.push(bytes[i]);
                        i += 1;
                        state = State::Normal;
                    }
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            State::LineComment => {
                out.push(bytes[i]);
                i += 1;
                if out.last() == Some(&b'\n') {
                    state = State::Normal;
                }
            }
            State::BlockComment => {
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    out.extend_from_slice(b"*/");
                    i += 2;
                    state = State::Normal;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
        }
    }

    String::from_utf8(out).expect("rewriting should preserve UTF-8")
}
