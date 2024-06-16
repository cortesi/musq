use crate::{sqlite::statement::CompoundStatement, Result};
use hashlink::lru_cache::LruCache;

const CAPACITY: usize = 1024;

/// A cache for prepared statements. When full, the least recently used
/// statement gets removed.
#[derive(Debug)]
pub struct StatementCache {
    inner: LruCache<String, CompoundStatement>,
}

impl StatementCache {
    /// Create a new cache with the given capacity.
    pub fn new() -> Self {
        Self {
            inner: LruCache::new(CAPACITY),
        }
    }

    pub fn get(&mut self, query: &str) -> Result<&mut CompoundStatement> {
        let exists = self.contains_key(query);
        if !exists {
            let statement = CompoundStatement::new(query)?;
            self.insert(query, statement);
        }
        let statement = self.get_mut(query).unwrap();
        if exists {
            // as this statement has been executed before, we reset before continuing
            statement.reset()?;
        }
        Ok(statement)
    }

    /// Returns a mutable reference to the value corresponding to the given key
    /// in the cache, if any.
    pub fn get_mut(&mut self, k: &str) -> Option<&mut CompoundStatement> {
        self.inner.get_mut(k)
    }

    /// Inserts a new statement to the cache, returning the least recently used
    /// statement id if the cache is full, or if inserting with an existing key,
    /// the replaced existing statement.
    pub fn insert(&mut self, k: &str, v: CompoundStatement) -> Option<CompoundStatement> {
        let mut lru_item = None;

        if self.capacity() == self.len() && !self.contains_key(k) {
            lru_item = self.remove_lru();
        } else if self.contains_key(k) {
            lru_item = self.inner.remove(k);
        }

        self.inner.insert(k.into(), v);

        lru_item
    }

    /// The number of statements in the cache.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Removes the least recently used item from the cache.
    pub fn remove_lru(&mut self) -> Option<CompoundStatement> {
        self.inner.remove_lru().map(|(_, v)| v)
    }

    /// Clear all cached statements from the cache.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// True if cache has a value for the given key.
    pub fn contains_key(&mut self, k: &str) -> bool {
        self.inner.contains_key(k)
    }

    /// Returns the maximum number of statements the cache can hold.
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}
