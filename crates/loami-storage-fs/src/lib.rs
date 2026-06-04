//! # loami-storage-fs
//!
//! A local-filesystem [`StorageProvider`] (`file://`), rooted at a directory and built on
//! `object_store`'s `LocalFileSystem`. It maps the object-store surface onto Loami's
//! [`StorageProvider`] contract — including create-if-absent and (emulated) compare-and-swap — and
//! passes the shared [`conformance`](loami_storage::conformance) suite.
//!
//! Suitable for local development and single-node persistence.
//!
//! ## Conditional updates
//!
//! `object_store`'s `LocalFileSystem` implements atomic `Create` but not conditional `Update`
//! (compare-and-swap by ETag). This provider emulates `Update` by reading the current ETag and
//! overwriting only if it matches. To make that check-then-write atomic, **all** writes (`put` of
//! any mode, and `delete`) are serialized through a single in-process lock — so writes to unrelated
//! keys do not proceed concurrently. That throughput trade-off is acceptable under Loami's
//! single-writer model (one process owns a given store). It does **not** guard against a second OS
//! process writing the same directory concurrently; cross-process CAS via OS file locks is tracked
//! as a future enhancement.

use bytes::Bytes;
use futures::TryStreamExt;
use loami_storage::{
    Etag, GetResult, ObjectKey, ObjectMeta, PutMode, PutOptions, PutResult, Result, StorageError,
    StorageProvider,
};
use object_store::{path::Path, ObjectStore, ObjectStoreExt};

/// A [`StorageProvider`] backed by the local filesystem, rooted at a directory.
#[derive(Debug)]
pub struct FsProvider {
    store: object_store::local::LocalFileSystem,
    // Serializes writes within the process so the emulated conditional `Update` is atomic.
    write_lock: futures::lock::Mutex<()>,
}

impl FsProvider {
    /// Creates a provider rooted at `root`. The directory must already exist.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError::Backend`] if the root cannot be opened.
    pub fn new(root: impl AsRef<std::path::Path>) -> Result<Self> {
        let store = object_store::local::LocalFileSystem::new_with_prefix(root).map_err(backend)?;
        Ok(Self {
            store,
            write_lock: futures::lock::Mutex::new(()),
        })
    }
}

/// Wraps an arbitrary object-store error as a backend error.
fn backend(err: object_store::Error) -> StorageError {
    StorageError::Backend {
        source: Box::new(err),
    }
}

/// Maps an object-store error to the contract's error type, preserving the conditional-write and
/// not-found cases that the conformance suite checks for.
fn map_err(key: &ObjectKey, err: object_store::Error) -> StorageError {
    match err {
        object_store::Error::NotFound { .. } => StorageError::NotFound { key: key.clone() },
        object_store::Error::AlreadyExists { .. } => {
            StorageError::AlreadyExists { key: key.clone() }
        }
        object_store::Error::Precondition { .. } => StorageError::Precondition { key: key.clone() },
        other => backend(other),
    }
}

/// Converts an object-store [`object_store::ObjectMeta`] to the contract's [`ObjectMeta`].
fn to_meta(meta: object_store::ObjectMeta) -> Result<ObjectMeta> {
    let object_store::ObjectMeta {
        location,
        last_modified,
        size,
        e_tag,
        ..
    } = meta;
    let etag = e_tag.map(Etag::new).ok_or_else(|| StorageError::Backend {
        source: format!("object store returned no etag for {location}").into(),
    })?;
    Ok(ObjectMeta {
        key: ObjectKey::new(location.to_string()),
        size,
        etag,
        last_modified: Some(last_modified.into()),
    })
}

#[async_trait::async_trait]
impl StorageProvider for FsProvider {
    async fn get(&self, key: &ObjectKey) -> Result<GetResult> {
        key.validate()?;
        let path = Path::from(key.as_str());
        let result = self.store.get(&path).await.map_err(|e| map_err(key, e))?;
        let meta = to_meta(result.meta.clone())?;
        let data = result.bytes().await.map_err(|e| map_err(key, e))?;
        Ok(GetResult { data, meta })
    }

    async fn get_range(&self, key: &ObjectKey, range: std::ops::Range<u64>) -> Result<GetResult> {
        key.validate()?;
        let path = Path::from(key.as_str());
        // object_store does not guarantee a uniform error for an out-of-bounds range, so validate
        // against the object's size first. The head also supplies the metadata for the result.
        let head = self.store.head(&path).await.map_err(|e| map_err(key, e))?;
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
                meta: to_meta(head)?,
            });
        }
        let data = self
            .store
            .get_range(&path, range)
            .await
            .map_err(|e| map_err(key, e))?;
        Ok(GetResult {
            data,
            meta: to_meta(head)?,
        })
    }

    async fn head(&self, key: &ObjectKey) -> Result<ObjectMeta> {
        key.validate()?;
        let path = Path::from(key.as_str());
        let meta = self.store.head(&path).await.map_err(|e| map_err(key, e))?;
        to_meta(meta)
    }

    async fn put(&self, key: &ObjectKey, data: Bytes, options: PutOptions) -> Result<PutResult> {
        key.validate()?;
        let path = Path::from(key.as_str());
        let _guard = self.write_lock.lock().await;

        // LocalFileSystem does not implement PutMode::Update, so emulate compare-and-swap under the
        // write lock: verify the current ETag, then fall through to an unconditional overwrite.
        if let PutMode::Update { expected } = &options.mode {
            match self.store.head(&path).await {
                Ok(meta) => {
                    let current = meta.e_tag.ok_or_else(|| StorageError::Backend {
                        source: format!("object store returned no etag for {key}").into(),
                    })?;
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
        let result = self
            .store
            .put_opts(&path, data.into(), os_mode.into())
            .await
            .map_err(|e| map_err(key, e))?;
        let etag = result
            .e_tag
            .map(Etag::new)
            .ok_or_else(|| StorageError::Backend {
                source: format!("object store returned no etag for {key}").into(),
            })?;
        Ok(PutResult { etag })
    }

    async fn delete(&self, key: &ObjectKey) -> Result<()> {
        key.validate()?;
        let path = Path::from(key.as_str());
        let _guard = self.write_lock.lock().await;
        match self.store.delete(&path).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(map_err(key, e)),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>> {
        let prefix_path = Path::from(prefix);
        let metas: Vec<object_store::ObjectMeta> = self
            .store
            .list(Some(&prefix_path))
            .try_collect()
            .await
            .map_err(backend)?;
        metas.into_iter().map(to_meta).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn passes_storage_conformance_suite() {
        let dir = tempfile::tempdir().unwrap();
        let provider = FsProvider::new(dir.path()).unwrap();
        loami_storage::conformance::run_conformance_suite(&provider).await;
    }

    #[tokio::test]
    async fn data_persists_across_provider_instances() {
        let dir = tempfile::tempdir().unwrap();
        let key = ObjectKey::new("persist/value");

        {
            let provider = FsProvider::new(dir.path()).unwrap();
            provider
                .put(
                    &key,
                    Bytes::from_static(b"durable"),
                    PutOptions::overwrite(),
                )
                .await
                .unwrap();
        }

        // A fresh provider rooted at the same directory sees the previously written object.
        let reopened = FsProvider::new(dir.path()).unwrap();
        assert_eq!(
            reopened.get(&key).await.unwrap().data,
            Bytes::from_static(b"durable")
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_compare_and_swap_lets_exactly_one_win() {
        let dir = tempfile::tempdir().unwrap();
        let provider = std::sync::Arc::new(FsProvider::new(dir.path()).unwrap());
        let key = ObjectKey::new("race/key");
        let seed = provider
            .put(&key, Bytes::from_static(b"0"), PutOptions::overwrite())
            .await
            .unwrap();

        // Two updates race from the same expected ETag; the in-process lock must let only one win.
        let one = {
            let (p, k, e) = (provider.clone(), key.clone(), seed.etag.clone());
            tokio::spawn(async move {
                p.put(&k, Bytes::from_static(b"1"), PutOptions::update(e))
                    .await
            })
        };
        let two = {
            let (p, k, e) = (provider.clone(), key.clone(), seed.etag.clone());
            tokio::spawn(async move {
                p.put(&k, Bytes::from_static(b"2"), PutOptions::update(e))
                    .await
            })
        };

        let winners = [one.await.unwrap(), two.await.unwrap()]
            .iter()
            .filter(|r| r.is_ok())
            .count();
        assert_eq!(
            winners, 1,
            "exactly one concurrent compare-and-swap must succeed"
        );
    }
}
