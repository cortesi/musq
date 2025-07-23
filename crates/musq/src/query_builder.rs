use crate::{
    Arguments, Result, Row,
    encode::Encode,
    executor::Execute,
    query::{Map, Query, query_with},
};

/// Builder for constructing SQL queries programmatically.
pub struct QueryBuilder {
    sql: String,
    args: Arguments,
    tainted: bool,
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            sql: String::new(),
            args: Arguments::default(),
            tainted: false,
        }
    }

    /// Create a builder from an existing [`Query`].
    pub fn from_query(mut query: Query) -> Self {
        let sql = query.sql().to_string();
        let args = query.take_arguments().unwrap_or_default();
        let tainted = query.tainted;
        Self { sql, args, tainted }
    }

    /// Push a raw SQL string fragment.
    pub fn push_sql(&mut self, sql: &str) {
        self.sql.push_str(sql);
    }

    /// Push a raw SQL fragment marking the builder as tainted.
    pub fn push_raw(&mut self, raw: &str) {
        self.tainted = true;
        self.sql.push_str(raw);
    }

    /// Append an identifier quoted for SQLite.
    pub fn push_identifier(&mut self, ident: &str) {
        self.sql.push('"');
        for c in ident.chars() {
            if c == '"' {
                self.sql.push('"');
            }
            self.sql.push(c);
        }
        self.sql.push('"');
    }

    /// Append a comma separated list of identifiers.
    pub fn push_idents<I>(&mut self, idents: I)
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut iter = idents.into_iter().peekable();
        while let Some(ident) = iter.next() {
            self.push_identifier(ident.as_ref());
            if iter.peek().is_some() {
                self.sql.push_str(", ");
            }
        }
    }

    /// Append a placeholder and bind a single value.
    pub fn push_bind<T: Encode>(&mut self, value: T) {
        self.sql.push('?');
        let _ = self.args.add(value);
    }

    /// Append a named placeholder and bind a single value.
    pub fn push_bind_named<T: Encode>(&mut self, name: &str, value: T) {
        self.sql.push(':');
        self.sql.push_str(name);
        let _ = self.args.add_named(name, value);
    }

    /// Append a comma separated list of placeholders for multiple values.
    pub fn push_values<I, T>(&mut self, values: I)
    where
        I: IntoIterator<Item = T>,
        T: Encode,
    {
        let mut iter = values.into_iter().peekable();
        while let Some(v) = iter.next() {
            self.push_bind(v);
            if iter.peek().is_some() {
                self.sql.push_str(", ");
            }
        }
    }

    /// Build the final [`Query`].
    pub fn build(self) -> Query {
        let mut q = query_with(&self.sql, self.args);
        q.tainted = self.tainted;
        q
    }

    /// Build a mapped query returning type `O`.
    pub fn build_map<O>(self) -> Map<impl FnMut(Row) -> Result<O> + Send>
    where
        O: Send + Unpin + for<'r> crate::from_row::FromRow<'r>,
    {
        let mut q = query_with(&self.sql, self.args);
        q.tainted = self.tainted;
        q.try_map(|row| O::from_row("", &row))
    }
}
