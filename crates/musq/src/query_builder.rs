use std::collections::HashSet;

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
            self.sql.push('?');
            self.arguments.values.push(val.clone());
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
        for (k, v) in values.iter() {
            if !first {
                self.sql.push_str(", ");
            }
            first = false;
            self.sql.push_str(&crate::quote_identifier(k));
            self.sql.push_str(" = ?");
            self.arguments.values.push(v.clone());
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
        for (k, v) in values.iter() {
            if !first {
                self.sql.push_str(" AND ");
            }
            first = false;
            self.sql.push_str(&crate::quote_identifier(k));
            self.sql.push_str(" = ?");
            self.arguments.values.push(v.clone());
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
            self.sql.push_str(query.sql());

            if let Some(other_args) = query.arguments {
                let base_index = self.arguments.values.len();
                self.arguments.values.extend(other_args.values);
                for (name, index) in other_args.named {
                    self.arguments.named.insert(name, base_index + index);
                }
            }

            self.tainted |= query.tainted;
        }
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
