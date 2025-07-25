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
}
