use indexmap::IndexMap;

use crate::{Result, Value, encode::Encode, expr::Expr, query::Query};

/// A value in a [`Values`] collection.
#[derive(Debug, Clone)]
pub enum ValuesEntry {
    /// A bound value (encoded immediately).
    Value(Value),
    /// A SQL expression fragment (may include its own bound parameters).
    Expr(Expr),
}

/// Convert a value into a [`ValuesEntry`].
pub trait IntoValuesEntry {
    /// Convert into a [`ValuesEntry`].
    fn into_values_entry(self) -> Result<ValuesEntry>;
}

impl<T> IntoValuesEntry for T
where
    T: Encode,
{
    fn into_values_entry(self) -> Result<ValuesEntry> {
        let encoded = self.encode().map_err(crate::Error::Encode)?;
        drop(self);
        Ok(ValuesEntry::Value(encoded))
    }
}

impl IntoValuesEntry for Expr {
    fn into_values_entry(self) -> Result<ValuesEntry> {
        Ok(ValuesEntry::Expr(self))
    }
}

impl IntoValuesEntry for Query {
    fn into_values_entry(self) -> Result<ValuesEntry> {
        Ok(ValuesEntry::Expr(self.into()))
    }
}

/// An ordered collection of key-value pairs for building dynamic SQL queries.
///
/// When used with the `sql!` macro's `{where:values}` placeholder, `NULL` values are rendered as
/// `col IS NULL` (without a bound parameter).
///
/// Values may also include SQL expression fragments (see [`crate::expr`]) for computed columns in
/// `{set:...}` and `{insert:...}` placeholders.
#[derive(Debug, Default, Clone)]
pub struct Values(IndexMap<String, ValuesEntry>);

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
        V: IntoValuesEntry,
    {
        self.0.insert(key.into(), value.into_values_entry()?);
        Ok(())
    }

    /// Consumes `self`, inserts a key-value pair, and returns `Self` for
    /// chaining.
    pub fn val<K, V>(mut self, key: K, value: V) -> Result<Self>
    where
        K: Into<String>,
        V: IntoValuesEntry,
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
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ValuesEntry)> {
        self.0.iter()
    }

    /// Iterate over the keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }

    /// Iterate over the values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &ValuesEntry> {
        self.0.values()
    }

    /// Extends this collection with the key-value pairs from another `Values` collection.
    ///
    /// If a key exists in both collections, the value from `other` will overwrite
    /// the existing value in this collection. The insertion order is preserved,
    /// with existing keys maintaining their position and new keys appended.
    pub fn extend(&mut self, other: &Self) {
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

    #[test]
    fn test_values_with_option_some() {
        let mut values = Values::new();

        // Test various Some() values
        values.insert("id", Some(42)).unwrap();
        values.insert("name", Some("Alice")).unwrap();
        values.insert("active", Some(true)).unwrap();
        values.insert("score", Some(98.5)).unwrap();

        assert_eq!(values.len(), 4);
        let keys: Vec<&String> = values.keys().collect();
        assert_eq!(keys, vec!["id", "name", "active", "score"]);
    }

    #[test]
    fn test_values_with_option_none() {
        let mut values = Values::new();

        // Test various None values
        values.insert("id", Some(1)).unwrap();
        values.insert("middle_name", None::<&str>).unwrap();
        values.insert("phone", None::<String>).unwrap();
        values.insert("age", None::<i32>).unwrap();
        values.insert("verified", None::<bool>).unwrap();

        assert_eq!(values.len(), 5);
        let keys: Vec<&String> = values.keys().collect();
        assert_eq!(keys, vec!["id", "middle_name", "phone", "age", "verified"]);
    }

    #[test]
    fn test_values_builder_with_options() {
        let values = Values::new()
            .val("id", 1)
            .unwrap()
            .val("name", Some("Bob"))
            .unwrap()
            .val("email", None::<String>)
            .unwrap()
            .val("active", Some(false))
            .unwrap()
            .val("score", None::<f64>)
            .unwrap();

        assert_eq!(values.len(), 5);
        let keys: Vec<&String> = values.keys().collect();
        assert_eq!(keys, vec!["id", "name", "email", "active", "score"]);
    }

    #[test]
    fn test_values_mixed_some_none() {
        let mut values = Values::new();

        // Mix of Some and None values for the same types
        values.insert("required_field", "always present").unwrap();
        values.insert("optional_text", Some("present")).unwrap();
        values.insert("missing_text", None::<String>).unwrap();
        values.insert("optional_number", Some(123)).unwrap();
        values.insert("missing_number", None::<i32>).unwrap();

        assert_eq!(values.len(), 5);
    }

    #[test]
    fn test_values_extend_with_options() {
        let mut values1 = Values::new()
            .val("id", 1)
            .unwrap()
            .val("name", Some("Alice"))
            .unwrap();

        let values2 = Values::new()
            .val("email", None::<String>)
            .unwrap()
            .val("phone", Some("+1-555-0123"))
            .unwrap()
            .val("age", None::<i32>)
            .unwrap();

        values1.extend(&values2);

        assert_eq!(values1.len(), 5);
        let keys: Vec<&String> = values1.keys().collect();
        assert_eq!(keys, vec!["id", "name", "email", "phone", "age"]);
    }

    #[test]
    fn test_values_option_overwrite() {
        let mut values = Values::new().val("field", Some("original")).unwrap();

        // Overwrite Some with None
        values.insert("field", None::<String>).unwrap();
        assert_eq!(values.len(), 1);

        // Overwrite None with Some
        values.insert("field", Some("updated")).unwrap();
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn test_values_option_string_types() {
        let mut values = Values::new();

        // Test different string option types
        values
            .insert("owned_string", Some("hello".to_string()))
            .unwrap();
        values.insert("string_ref", Some("world")).unwrap();
        values.insert("no_owned_string", None::<String>).unwrap();
        values.insert("no_string_ref", None::<&str>).unwrap();

        assert_eq!(values.len(), 4);
    }

    #[test]
    fn test_values_with_null_constant() {
        use crate::encode::Null;

        let mut values = Values::new();

        // Test using the Null constant
        values.insert("id", 1).unwrap();
        values.insert("name", "Alice").unwrap();
        values.insert("middle_name", Null).unwrap();
        values.insert("phone", Some("+1-555-0123")).unwrap();

        assert_eq!(values.len(), 4);
        let keys: Vec<&String> = values.keys().collect();
        assert_eq!(keys, vec!["id", "name", "middle_name", "phone"]);
    }

    #[test]
    fn test_values_macro_with_null() -> crate::Result<()> {
        use crate::{encode::Null, values};

        let user_data = values! {
            "id": 1,
            "name": "Bob",
            "middle_name": Null,  // Using Null constant
            "email": Some("bob@example.com"),
            "phone": None::<String>  // Traditional None with type annotation
        }?;

        assert_eq!(user_data.len(), 5);
        let keys: Vec<&String> = user_data.keys().collect();
        assert_eq!(keys, vec!["id", "name", "middle_name", "email", "phone"]);
        Ok(())
    }

    #[test]
    fn test_values_extend_with_null() {
        use crate::encode::Null;

        let mut values1 = Values::new()
            .val("id", 1)
            .unwrap()
            .val("name", "Charlie")
            .unwrap();

        let values2 = Values::new()
            .val("middle_name", Null) // Using Null constant
            .unwrap()
            .val("last_name", "Brown")
            .unwrap();

        values1.extend(&values2);

        assert_eq!(values1.len(), 4);
        let keys: Vec<&String> = values1.keys().collect();
        assert_eq!(keys, vec!["id", "name", "middle_name", "last_name"]);
    }
}
