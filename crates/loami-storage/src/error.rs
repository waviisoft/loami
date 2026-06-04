//! Error and result types for the storage contract.

use crate::ObjectKey;

/// A specialized result type for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;

/// The error type returned by every [`StorageProvider`](crate::StorageProvider) operation.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StorageError {
    /// No object exists at the requested key.
    #[error("object not found: {key}")]
    NotFound {
        /// The key that was not found.
        key: ObjectKey,
    },

    /// A [`PutMode::Create`](crate::PutMode::Create) write found an existing object.
    #[error("object already exists: {key}")]
    AlreadyExists {
        /// The key that already existed.
        key: ObjectKey,
    },

    /// A [`PutMode::Update`](crate::PutMode::Update) write failed its ETag precondition.
    #[error("precondition failed (etag mismatch): {key}")]
    Precondition {
        /// The key whose precondition failed.
        key: ObjectKey,
    },

    /// A range read fell outside the object's bounds.
    #[error("invalid range {start}..{end} for object of size {size}: {key}")]
    InvalidRange {
        /// The key being read.
        key: ObjectKey,
        /// The requested (inclusive) start offset.
        start: u64,
        /// The requested (exclusive) end offset.
        end: u64,
        /// The object's actual size in bytes.
        size: u64,
    },

    /// The provider does not support the requested operation.
    #[error("operation not supported by this provider: {operation}")]
    Unsupported {
        /// The name of the unsupported operation.
        operation: &'static str,
    },

    /// The key is not well-formed (see [`ObjectKey::validate`](crate::ObjectKey::validate)).
    #[error("invalid object key {key:?}: {reason}")]
    InvalidKey {
        /// The offending key.
        key: String,
        /// Why the key was rejected.
        reason: &'static str,
    },

    /// An error originating from the underlying backend.
    #[error("storage backend error: {source}")]
    Backend {
        /// The underlying backend error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
