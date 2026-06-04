//! The [`StorageProvider`] trait — the contract every backend implements.

use std::ops::Range;

use bytes::Bytes;

use crate::{GetResult, ObjectKey, ObjectMeta, PutOptions, PutResult, Result};

/// An object store that Loami can persist to and read from.
///
/// This is the seam between the engine and any concrete backend. Implementations are expected to be
/// cheaply cloneable behind an `Arc` and safe to share across tasks (`Send + Sync`). Every
/// implementation must pass the [`conformance`](crate::conformance) suite.
///
/// Keys are forward-slash-separated paths (see [`ObjectKey`]). Every key-addressed method validates
/// its key with [`ObjectKey::validate`](crate::ObjectKey::validate) and returns
/// [`StorageError::InvalidKey`](crate::StorageError::InvalidKey) before touching the backend. All
/// methods are asynchronous; the engine drives them on its own runtime.
#[async_trait::async_trait]
pub trait StorageProvider: Send + Sync {
    /// Reads the full contents of the object at `key`, together with its metadata.
    ///
    /// The returned [`GetResult::meta`] carries the ETag of exactly the bytes returned, so a caller
    /// can perform a subsequent conditional write without a separate [`head`](Self::head) call (and
    /// without the race that a read-then-head pair would introduce).
    ///
    /// Returns [`StorageError::NotFound`](crate::StorageError::NotFound) if no object exists.
    async fn get(&self, key: &ObjectKey) -> Result<GetResult>;

    /// Reads the half-open byte range `range` (`start..end`) of the object at `key`, together with
    /// the object's metadata.
    ///
    /// [`GetResult::data`] is the requested slice; an empty range (`start == end`, within bounds)
    /// returns empty data. [`GetResult::meta`] describes the whole object (including its current
    /// ETag). Unlike [`get`](Self::get), a provider may read the metadata
    /// separately from the bytes, so under concurrent modification the two could reflect different
    /// versions; this does not arise under Loami's single-writer model.
    ///
    /// Returns [`StorageError::NotFound`](crate::StorageError::NotFound) if no object exists, or
    /// [`StorageError::InvalidRange`](crate::StorageError::InvalidRange) if the range is malformed or
    /// extends beyond the object.
    async fn get_range(&self, key: &ObjectKey, range: Range<u64>) -> Result<GetResult>;

    /// Returns metadata (size, ETag, last-modified) for the object at `key` without its body.
    ///
    /// Returns [`StorageError::NotFound`](crate::StorageError::NotFound) if no object exists.
    async fn head(&self, key: &ObjectKey) -> Result<ObjectMeta>;

    /// Writes `data` to `key` according to `options`, returning the new object's ETag.
    ///
    /// See [`PutMode`](crate::PutMode) for the conditional-write semantics.
    async fn put(&self, key: &ObjectKey, data: Bytes, options: PutOptions) -> Result<PutResult>;

    /// Deletes the object at `key`.
    ///
    /// Deleting a key that does not exist succeeds (the operation is idempotent).
    async fn delete(&self, key: &ObjectKey) -> Result<()>;

    /// Lists metadata for all objects under `prefix`, matched on `/`-segment boundaries
    /// (directory-style), not as a raw string prefix. A trailing `/` on `prefix` is ignored, and an
    /// empty `prefix` lists every object. For example, `list("a/b")` returns `a/b/c` but not
    /// `a/bc`. Whether an object whose key is exactly `prefix` is included is unspecified; the
    /// engine never lists a prefix that is also an object key.
    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>>;
}
