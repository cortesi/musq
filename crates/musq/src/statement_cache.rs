use hashlink::lru_cache::LruCache;

use crate::{Result, sqlite::statement::CompoundStatement};

/// Default capacity for [`StatementCache`].
pub const DEFAULT_CAPACITY: usize = 1024;

/// A cache for prepared statements. When full, the least recently used
/// statement gets removed.
#[derive(Debug)]
pub struct StatementCache {
    /// LRU cache keyed by SQL string.
    inner: LruCache<String, CompoundStatement>,
}

impl StatementCache {
    /// Create a new cache with the given `capacity`.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: LruCache::new(capacity),
        }
    }

    /// Get a cached statement for the given query, preparing it if needed.
    pub fn get(&mut self, query: &str) -> Result<&mut CompoundStatement> {
        let exists = self.inner.contains_key(query);
        if !exists {
            let statement = CompoundStatement::new(query)?;
            self.inner.insert(query.to_owned(), statement);
        }
        let statement = self
            .inner
            .get_mut(query)
            .expect("statement was just inserted");
        if exists {
            // as this statement has been executed before, we reset before
            // continuing
            statement.reset()?;
        }
        Ok(statement)
    }

    /// The number of statements in the cache.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Clear all cached statements from the cache.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Connection, Musq, query_as};

    #[tokio::test]
    async fn test_cached_statement_reused_with_different_args() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;

        let initial = conn.cached_statements_size();

        let (v1,): (i32,) = query_as("SELECT ?1").bind(1_i32).fetch_one(&conn).await?;
        assert_eq!(v1, 1);
        assert_eq!(conn.cached_statements_size(), initial + 1);

        let (v2,): (i32,) = query_as("SELECT ?1").bind(5_i32).fetch_one(&conn).await?;
        assert_eq!(v2, 5);
        assert_eq!(conn.cached_statements_size(), initial + 1);

        conn.clear_cached_statements().await?;
        assert_eq!(conn.cached_statements_size(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_statement_cache_get_returns_same_statement() -> anyhow::Result<()> {
        let mut cache = StatementCache::new(DEFAULT_CAPACITY);

        let ptr_first: *const CompoundStatement = {
            let stmt = cache.get("SELECT 1")?;
            stmt as *const _
        };
        let ptr_second: *const CompoundStatement = {
            let stmt = cache.get("SELECT 1")?;
            stmt as *const _
        };

        assert_eq!(ptr_first, ptr_second);

        Ok(())
    }
}
