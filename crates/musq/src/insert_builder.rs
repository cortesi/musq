use crate::query::Query;
use crate::{
    Arguments, Connection, Pool, QueryResult, Result, encode::Encode, query::query_with,
    quote_identifier,
};

/// Builder for constructing `INSERT INTO` queries.
pub struct InsertInto {
    table: String,
    columns: Vec<String>,
    arguments: Arguments,
}

/// Create a new [`InsertInto`] builder for the given table.
#[allow(non_snake_case)]
pub fn insert_into(table: &str) -> InsertInto {
    InsertInto {
        table: quote_identifier(table),
        columns: Vec::new(),
        arguments: Arguments::default(),
    }
}

impl InsertInto {
    /// Add a column/value pair to the `INSERT` statement.
    pub fn value<T: Encode>(mut self, column: &str, value: T) -> Self {
        self.columns.push(quote_identifier(column));
        let _ = self.arguments.add(value);
        self
    }

    /// Build the final [`Query`].
    pub fn query(self) -> Result<Query> {
        if self.columns.is_empty() {
            return Err(crate::Error::Protocol("Insert query has no values".into()));
        }
        let columns = self.columns.join(", ");
        let placeholders = (0..self.columns.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            self.table, columns, placeholders
        );
        Ok(query_with(&sql, self.arguments))
    }

    /// Build and execute the query using a [`Connection`].
    pub async fn execute(self, conn: &Connection) -> Result<QueryResult> {
        let q = self.query()?;
        q.execute(conn).await
    }

    /// Build and execute the query using a [`Pool`].
    pub async fn execute_on_pool(self, pool: &Pool) -> Result<QueryResult> {
        let q = self.query()?;
        q.execute(pool).await
    }
}
