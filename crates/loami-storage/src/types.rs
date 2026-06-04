//! Shared value types used across the storage contract.

use std::fmt;
use std::time::SystemTime;

use bytes::Bytes;

use crate::{Result, StorageError};

/// The key (path) identifying an object within a provider's namespace.
///
/// Keys are UTF-8 strings using forward slashes (`/`) as separators, e.g. `wal/000042`. They are
/// opaque to the contract; providers map them onto their backend's native naming.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectKey(String);

impl ObjectKey {
    /// Creates a key from anything string-like.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Returns the key as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validates that the key is well-formed and identical across every provider (no
    /// backend-specific normalization). A valid key is a non-empty sequence of `/`-separated
    /// segments where each segment is non-empty, is neither `.` nor `..`, and contains only the
    /// characters `[A-Za-z0-9._-]`. There is no leading, trailing, or repeated `/`.
    ///
    /// Providers reject invalid keys with [`StorageError::InvalidKey`] before touching the backend.
    /// This guarantees keys round-trip unchanged and bars path-traversal segments.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidKey`] describing the first violation found.
    pub fn validate(&self) -> Result<()> {
        let key = &self.0;
        if key.is_empty() {
            return Err(self.invalid("key must not be empty"));
        }
        if key.starts_with('/') || key.ends_with('/') {
            return Err(self.invalid("key must not start or end with '/'"));
        }
        for segment in key.split('/') {
            if segment.is_empty() {
                return Err(self.invalid("key must not contain an empty segment ('//')"));
            }
            if segment == "." || segment == ".." {
                return Err(self.invalid("key segments must not be '.' or '..'"));
            }
            if !segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            {
                return Err(self.invalid("key segments may contain only [A-Za-z0-9._-]"));
            }
        }
        Ok(())
    }

    fn invalid(&self, reason: &'static str) -> StorageError {
        StorageError::InvalidKey {
            key: self.0.clone(),
            reason,
        }
    }
}

impl fmt::Display for ObjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ObjectKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ObjectKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// An opaque entity tag identifying a specific version of an object.
///
/// ETags are produced by a provider on write ([`PutResult::etag`]) and supplied back on a
/// conditional update ([`PutMode::Update`](crate::PutMode::Update)) to implement optimistic
/// concurrency. They are compared only for equality; their internal format is provider-defined.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Etag(String);

impl Etag {
    /// Creates an ETag from a provider-supplied token.
    pub fn new(tag: impl Into<String>) -> Self {
        Self(tag.into())
    }

    /// Returns the ETag as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Etag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Metadata describing a stored object, as returned by
/// [`head`](crate::StorageProvider::head) and [`list`](crate::StorageProvider::list).
#[derive(Clone, Debug)]
pub struct ObjectMeta {
    /// The object's key.
    pub key: ObjectKey,
    /// The object's size in bytes.
    pub size: u64,
    /// The object's current ETag.
    pub etag: Etag,
    /// When the object was last modified, if the provider reports it.
    pub last_modified: Option<SystemTime>,
}

/// How a [`put`](crate::StorageProvider::put) should behave with respect to any existing object.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PutMode {
    /// Unconditionally write, replacing any existing object.
    #[default]
    Overwrite,
    /// Write only if no object currently exists at the key.
    ///
    /// Fails with [`StorageError::AlreadyExists`](crate::StorageError::AlreadyExists) on conflict.
    Create,
    /// Write only if the current object's ETag equals `expected` (compare-and-swap).
    ///
    /// Fails with [`StorageError::Precondition`](crate::StorageError::Precondition) if the ETag does
    /// not match (or, for some providers, if no object exists).
    Update {
        /// The ETag the caller expects the current object to have.
        expected: Etag,
    },
}

/// Options controlling a [`put`](crate::StorageProvider::put) operation.
#[derive(Clone, Debug, Default)]
pub struct PutOptions {
    /// The conditional-write mode.
    pub mode: PutMode,
}

impl PutOptions {
    /// Unconditional overwrite.
    #[must_use]
    pub fn overwrite() -> Self {
        Self {
            mode: PutMode::Overwrite,
        }
    }

    /// Write only if the key does not already exist.
    #[must_use]
    pub fn create() -> Self {
        Self {
            mode: PutMode::Create,
        }
    }

    /// Write only if the current ETag matches `expected` (compare-and-swap).
    #[must_use]
    pub fn update(expected: Etag) -> Self {
        Self {
            mode: PutMode::Update { expected },
        }
    }
}

/// The outcome of a successful [`put`](crate::StorageProvider::put).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PutResult {
    /// The ETag of the newly written object, for use in a subsequent conditional update.
    pub etag: Etag,
}

/// The result of a read: the bytes together with the object's metadata.
///
/// Returning metadata alongside the body lets a caller capture the ETag of *exactly* the bytes it
/// read, in a single operation. This avoids a separate [`head`](crate::StorageProvider::head) call
/// and the read-then-head race that would otherwise let the ETag drift between the two calls before a
/// follow-up conditional write ([`PutMode::Update`]).
#[derive(Clone, Debug)]
pub struct GetResult {
    /// The bytes read — the whole object for [`get`](crate::StorageProvider::get), or the requested
    /// slice for [`get_range`](crate::StorageProvider::get_range).
    pub data: Bytes,
    /// Metadata for the object as a whole: notably its current [`etag`](ObjectMeta::etag) (valid for
    /// a subsequent [`PutMode::Update`]) and its full [`size`](ObjectMeta::size). For a range read,
    /// `size` is the size of the whole object, not the length of `data`.
    pub meta: ObjectMeta,
}
