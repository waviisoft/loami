//! Document identity and the document type returned by queries.

use std::fmt;

use serde_json::Value;

/// The identifier of a document within a collection.
///
/// Ids are generated on [`insert`](crate::Collection::insert) and are valid object-key segments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocId(String);

impl DocId {
    /// Wraps an existing id string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Generates a fresh, unique id (a time-ordered UUID v7), so ids sort by creation time —
    /// friendlier for listing and future indexing on backends that order keys lexicographically.
    pub(crate) fn generate() -> Self {
        Self(uuid::Uuid::now_v7().to_string())
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for DocId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for DocId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// A document returned by a query: its id paired with its JSON value.
#[derive(Clone, Debug)]
pub struct Document {
    /// The document's id.
    pub id: DocId,
    /// The document's JSON value.
    pub value: Value,
}
