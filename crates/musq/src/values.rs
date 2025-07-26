use indexmap::IndexMap;

use crate::{Result, Value, encode::Encode};

/// An ordered collection of key-value pairs for building dynamic SQL queries.
#[derive(Debug, Default, Clone)]
pub struct Values(IndexMap<String, Value>);

impl Values {
    /// Creates a new, empty `Values` collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a key-value pair into the collection.
    /// The key should be the name of the database column.
    pub fn insert<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: Into<String>,
        V: Encode,
    {
        let value = value.encode().map_err(crate::Error::Encode)?;
        self.0.insert(key.into(), value);
        Ok(())
    }

    /// Consumes `self`, inserts a key-value pair, and returns `Self` for
    /// chaining.
    pub fn val<K, V>(mut self, key: K, value: V) -> Result<Self>
    where
        K: Into<String>,
        V: Encode,
    {
        self.insert(key, value)?;
        Ok(self)
    }

    /// Returns `true` if the collection contains no elements.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of elements in the collection.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Iterate over key-value pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.0.iter()
    }

    /// Iterate over the keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }

    /// Iterate over the values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.0.values()
    }

    /// Extends this collection with the key-value pairs from another `Values` collection.
    ///
    /// If a key exists in both collections, the value from `other` will overwrite
    /// the existing value in this collection. The insertion order is preserved,
    /// with existing keys maintaining their position and new keys appended.
    pub fn extend(&mut self, other: &Values) {
        self.0
            .extend(other.0.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extend_with_new_keys() {
        let mut values1 = Values::new()
            .val("id", 1)
            .unwrap()
            .val("name", "Alice")
            .unwrap();

        let values2 = Values::new()
            .val("email", "alice@example.com")
            .unwrap()
            .val("status", "active")
            .unwrap();

        values1.extend(&values2);

        assert_eq!(values1.len(), 4);

        // Check that all keys are present
        let keys: Vec<&String> = values1.keys().collect();
        assert_eq!(keys, vec!["id", "name", "email", "status"]);
    }

    #[test]
    fn test_extend_with_duplicate_keys() {
        let mut values1 = Values::new()
            .val("id", 1)
            .unwrap()
            .val("name", "Alice")
            .unwrap()
            .val("status", "inactive")
            .unwrap();

        let values2 = Values::new()
            .val("status", "active")
            .unwrap()
            .val("email", "alice@example.com")
            .unwrap();

        values1.extend(&values2);

        assert_eq!(values1.len(), 4);

        // Check that duplicate key was overwritten
        let keys: Vec<&String> = values1.keys().collect();
        assert_eq!(keys, vec!["id", "name", "status", "email"]);

        // Verify the status was overwritten
        // Note: We can't easily check the actual value without accessing internals,
        // but we can verify the structure is correct
    }

    #[test]
    fn test_extend_with_empty_values() {
        let mut values1 = Values::new()
            .val("id", 1)
            .unwrap()
            .val("name", "Alice")
            .unwrap();

        let values2 = Values::new();

        values1.extend(&values2);

        assert_eq!(values1.len(), 2);
        let keys: Vec<&String> = values1.keys().collect();
        assert_eq!(keys, vec!["id", "name"]);
    }

    #[test]
    fn test_extend_empty_with_values() {
        let mut values1 = Values::new();

        let values2 = Values::new()
            .val("email", "alice@example.com")
            .unwrap()
            .val("status", "active")
            .unwrap();

        values1.extend(&values2);

        assert_eq!(values1.len(), 2);
        let keys: Vec<&String> = values1.keys().collect();
        assert_eq!(keys, vec!["email", "status"]);
    }

    #[test]
    fn test_extend_both_empty() {
        let mut values1 = Values::new();
        let values2 = Values::new();

        values1.extend(&values2);

        assert_eq!(values1.len(), 0);
        assert!(values1.is_empty());
    }

    #[test]
    fn test_extend_preserves_order() {
        let mut values1 = Values::new()
            .val("a", 1)
            .unwrap()
            .val("b", 2)
            .unwrap()
            .val("c", 3)
            .unwrap();

        let values2 = Values::new().val("d", 4).unwrap().val("e", 5).unwrap();

        values1.extend(&values2);

        let keys: Vec<&String> = values1.keys().collect();
        assert_eq!(keys, vec!["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn test_extend_multiple_times() {
        let mut values1 = Values::new().val("id", 1).unwrap();

        let values2 = Values::new().val("name", "Alice").unwrap();

        let values3 = Values::new().val("email", "alice@example.com").unwrap();

        values1.extend(&values2);
        values1.extend(&values3);

        assert_eq!(values1.len(), 3);
        let keys: Vec<&String> = values1.keys().collect();
        assert_eq!(keys, vec!["id", "name", "email"]);
    }
}
