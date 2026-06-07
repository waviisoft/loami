//! Error and result types for the document store.

/// A specialized result type for Loami operations.
pub type Result<T> = std::result::Result<T, Error>;

/// The error type returned by the document store.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// An error from the underlying storage provider.
    #[error("storage error: {0}")]
    Storage(#[from] loami_storage::StorageError),

    /// A document could not be serialized to or deserialized from JSON.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A collection name is not well-formed.
    #[error("invalid collection name {name:?}: {reason}")]
    InvalidName {
        /// The offending name.
        name: String,
        /// Why it was rejected.
        reason: &'static str,
    },

    /// A connection string passed to [`Loami::connect`](crate::Loami::connect) was malformed.
    #[error("invalid connection string {url:?}: {reason}")]
    Url {
        /// The offending connection string.
        url: String,
        /// Why it was rejected.
        reason: &'static str,
    },

    /// A connection string used a scheme that no registered provider handles.
    #[error("unknown scheme {scheme:?} in {url:?}; registered schemes: {registered}")]
    UnknownScheme {
        /// The full connection string.
        url: String,
        /// The unrecognized scheme.
        scheme: String,
        /// The schemes that are registered, comma-separated.
        registered: String,
    },
}
