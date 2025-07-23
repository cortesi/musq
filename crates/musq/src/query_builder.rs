use crate::{Arguments, Result, encode::Encode, query::Query};
use either::Either;

#[derive(Default)]
pub struct QueryBuilder {
    pub(crate) sql: String,
    pub(crate) arguments: Arguments,
    pub(crate) tainted: bool,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_parts(sql: String, arguments: Arguments, tainted: bool) -> Self {
        Self {
            sql,
            arguments,
            tainted,
        }
    }

    pub fn push_sql(&mut self, sql: &str) {
        self.sql.push_str(sql);
    }

    pub fn push_raw(&mut self, raw: &str) {
        self.sql.push_str(raw);
        self.tainted = true;
    }

    pub fn push_bind<T: Encode>(&mut self, value: T) -> Result<()> {
        self.arguments.add(value)?;
        self.sql.push('?');
        Ok(())
    }

    pub fn push_bind_named<T: Encode>(&mut self, name: &str, value: T) -> Result<()> {
        self.arguments.add_named(name, value)?;
        self.sql.push(':');
        self.sql.push_str(name);
        Ok(())
    }

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
            self.arguments.add(v)?;
        }
        if first {
            return Err(crate::Error::Protocol("empty values".into()));
        }
        Ok(())
    }

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

    pub fn build(self) -> Query {
        Query {
            statement: Either::Left(self.sql),
            arguments: Some(self.arguments),
            tainted: self.tainted,
        }
    }
}
