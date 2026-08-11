use crate::{Query, QueryBuilder, Result};

/// A collection of typed query fragments joined as one `WHERE` clause.
#[derive(Default)]
pub struct Conditions {
    /// Conditions in insertion order.
    fragments: Vec<Query>,
}

impl Conditions {
    /// Create an empty condition collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one condition fragment.
    pub fn push(&mut self, condition: Query) -> &mut Self {
        self.fragments.push(condition);
        self
    }

    /// Add one condition fragment and return the collection.
    pub fn with(mut self, condition: Query) -> Self {
        self.push(condition);
        self
    }

    /// Returns `true` when the collection has no condition fragments.
    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }

    /// Build the complete clause, if this collection is not empty.
    pub(crate) fn into_query(self) -> Result<Option<Query>> {
        if self.fragments.is_empty() {
            return Ok(None);
        }

        let mut builder = QueryBuilder::new();
        builder.push_sql("WHERE");
        for (index, condition) in self.fragments.into_iter().enumerate() {
            if index > 0 {
                builder.push_sql(" AND");
            }
            builder.try_push_query(condition)?;
        }
        Ok(Some(builder.build()))
    }
}
