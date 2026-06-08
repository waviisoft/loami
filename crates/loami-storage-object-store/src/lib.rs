//! # loami-storage-object-store
//!
//! Shared building blocks for Loami storage providers built on the [`object_store`] crate
//! (`loami-storage-fs`, `loami-storage-azure`, and future S3/GCS/R2 providers). These generic
//! helpers map the `object_store` surface onto Loami's
//! [`StorageProvider`](loami_storage::StorageProvider) contract **once** — key validation, error
//! mapping, metadata conversion, range handling, and conditional writes — so each provider is a thin
//! adapter that supplies a store and delegates here.
//!
//! Two write helpers cover the two ways backends implement compare-and-swap:
//! [`put_native`] for stores whose `object_store` implementation supports conditional `Update`
//! (e.g. Azure Blob), and [`put_emulated`] for those that do not (e.g. the local filesystem), which
//! reads-then-overwrites and so must be serialized by the caller against [`delete`].
//!
//! This is an internal convenience for the in-repo providers, not part of the storage contract: a
//! third-party provider implements [`StorageProvider`](loami_storage::StorageProvider) directly and
//! need not depend on this crate.

use bytes::Bytes;
use futures::stream::BoxStream;
use futures::StreamExt;
use loami_storage::{
    Etag, GetResult, ObjectKey, ObjectMeta, PutMode, PutOptions, PutResult, Result, StorageError,
};
use object_store::{path::Path, ObjectStore, ObjectStoreExt};

/// Wraps an `object_store` error that carries no key context (e.g. a store-construction failure) as
/// a [`StorageError::Backend`]. Providers use this for errors raised outside a keyed operation, such
/// as in their constructors.
pub fn backend_error(err: object_store::Error) -> StorageError {
    StorageError::Backend {
        source: Box::new(err),
    }
}

/// Maps a keyed `object_store` error to the contract's error type, preserving the conditional-write
/// and not-found cases that the conformance suite checks for.
fn map_err(key: &ObjectKey, err: object_store::Error) -> StorageError {
    match err {
        object_store::Error::NotFound { .. } => StorageError::NotFound { key: key.clone() },
        object_store::Error::AlreadyExists { .. } => {
            StorageError::AlreadyExists { key: key.clone() }
        }
        object_store::Error::Precondition { .. } => StorageError::Precondition { key: key.clone() },
        other => backend_error(other),
    }
}

/// Converts an [`object_store::ObjectMeta`] to the contract's [`ObjectMeta`], failing if the backend
/// returned no ETag.
fn to_meta(meta: &object_store::ObjectMeta) -> Result<ObjectMeta> {
    let etag = meta
        .e_tag
        .clone()
        .map(Etag::new)
        .ok_or_else(|| StorageError::Backend {
            source: format!("object store returned no etag for {}", meta.location).into(),
        })?;
    Ok(ObjectMeta {
        key: ObjectKey::new(meta.location.to_string()),
        size: meta.size,
        etag,
        last_modified: Some(meta.last_modified.into()),
    })
}

/// Extracts the ETag from a write result, failing if the backend returned none.
fn etag_from(e_tag: Option<String>, key: &ObjectKey) -> Result<Etag> {
    e_tag.map(Etag::new).ok_or_else(|| StorageError::Backend {
        source: format!("object store returned no etag for {key}").into(),
    })
}

/// Reads a whole object plus its metadata. The ETag in the returned metadata belongs to exactly the
/// bytes read.
pub async fn get<S: ObjectStore>(store: &S, key: &ObjectKey) -> Result<GetResult> {
    key.validate()?;
    let path = Path::from(key.as_str());
    let result = store.get(&path).await.map_err(|e| map_err(key, e))?;
    let meta = to_meta(&result.meta)?;
    let data = result.bytes().await.map_err(|e| map_err(key, e))?;
    Ok(GetResult { data, meta })
}

/// Reads a byte range of an object, plus its metadata.
pub async fn get_range<S: ObjectStore>(
    store: &S,
    key: &ObjectKey,
    range: std::ops::Range<u64>,
) -> Result<GetResult> {
    key.validate()?;
    let path = Path::from(key.as_str());
    // object_store does not guarantee a uniform error for an out-of-bounds range, so validate against
    // the object's size first. The head also supplies the metadata for the result.
    let head = store.head(&path).await.map_err(|e| map_err(key, e))?;
    let size = head.size;
    if range.start > range.end || range.end > size {
        return Err(StorageError::InvalidRange {
            key: key.clone(),
            start: range.start,
            end: range.end,
            size,
        });
    }
    if range.start == range.end {
        // object_store rejects a zero-length range; the contract returns empty bytes.
        return Ok(GetResult {
            data: Bytes::new(),
            meta: to_meta(&head)?,
        });
    }
    let data = store
        .get_range(&path, range)
        .await
        .map_err(|e| map_err(key, e))?;
    Ok(GetResult {
        data,
        meta: to_meta(&head)?,
    })
}

/// Reads an object's metadata (size, ETag, last-modified) without its body.
pub async fn head<S: ObjectStore>(store: &S, key: &ObjectKey) -> Result<ObjectMeta> {
    key.validate()?;
    let path = Path::from(key.as_str());
    let meta = store.head(&path).await.map_err(|e| map_err(key, e))?;
    to_meta(&meta)
}

/// Writes using the backend's native conditional modes, including ETag compare-and-swap for
/// [`PutMode::Update`]. For backends whose `object_store` implementation supports all three modes.
pub async fn put_native<S: ObjectStore>(
    store: &S,
    key: &ObjectKey,
    data: Bytes,
    options: PutOptions,
) -> Result<PutResult> {
    key.validate()?;
    let path = Path::from(key.as_str());
    let mode = match options.mode {
        PutMode::Overwrite => object_store::PutMode::Overwrite,
        PutMode::Create => object_store::PutMode::Create,
        PutMode::Update { expected } => {
            object_store::PutMode::Update(object_store::UpdateVersion {
                e_tag: Some(expected.as_str().to_owned()),
                version: None,
            })
        }
    };
    let result = store
        .put_opts(&path, data.into(), mode.into())
        .await
        .map_err(|e| map_err(key, e))?;
    Ok(PutResult {
        etag: etag_from(result.e_tag, key)?,
    })
}

/// Writes, emulating conditional [`PutMode::Update`] (compare-and-swap by ETag) for backends whose
/// `object_store` implementation lacks it (e.g. the local filesystem): the current ETag is read and
/// the write proceeds only if it matches.
///
/// **The caller must serialize writes** — hold a process-wide lock across this call and [`delete`] —
/// so the check-then-write is atomic. This helper does no locking of its own.
pub async fn put_emulated<S: ObjectStore>(
    store: &S,
    key: &ObjectKey,
    data: Bytes,
    options: PutOptions,
) -> Result<PutResult> {
    key.validate()?;
    let path = Path::from(key.as_str());

    if let PutMode::Update { expected } = &options.mode {
        match store.head(&path).await {
            Ok(meta) => {
                let current = etag_from(meta.e_tag, key)?;
                if current.as_str() != expected.as_str() {
                    return Err(StorageError::Precondition { key: key.clone() });
                }
            }
            Err(object_store::Error::NotFound { .. }) => {
                return Err(StorageError::Precondition { key: key.clone() });
            }
            Err(e) => return Err(map_err(key, e)),
        }
    }

    let os_mode = match options.mode {
        PutMode::Create => object_store::PutMode::Create,
        PutMode::Overwrite | PutMode::Update { .. } => object_store::PutMode::Overwrite,
    };
    let result = store
        .put_opts(&path, data.into(), os_mode.into())
        .await
        .map_err(|e| map_err(key, e))?;
    Ok(PutResult {
        etag: etag_from(result.e_tag, key)?,
    })
}

/// Deletes `key`, treating a missing object as success (idempotent). A backend that emulates
/// conditional writes under a lock (see [`put_emulated`]) must hold that lock across this call.
pub async fn delete<S: ObjectStore>(store: &S, key: &ObjectKey) -> Result<()> {
    key.validate()?;
    let path = Path::from(key.as_str());
    match store.delete(&path).await {
        Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
        Err(e) => Err(map_err(key, e)),
    }
}

/// Lazily lists objects under `prefix` as a constant-memory stream, mapping each entry onto the
/// contract's [`ObjectMeta`].
pub fn list<'a, S: ObjectStore>(store: &'a S, prefix: &str) -> BoxStream<'a, Result<ObjectMeta>> {
    let prefix_path = Path::from(prefix);
    store
        .list(Some(&prefix_path))
        .map(|res| to_meta(&res.map_err(backend_error)?))
        .boxed()
}
