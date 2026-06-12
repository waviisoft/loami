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
use futures::stream::BoxStream;
use loami_storage::{
    FromUrl, GetResult, ObjectKey, ObjectMeta, PutOptions, PutResult, Result, StorageError,
    StorageProvider,
};
use loami_storage_object_store as adapter;

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
    /// Returns a [`StorageError::Backend`](loami_storage::StorageError::Backend) if the root cannot
    /// be opened.
    pub fn new(root: impl AsRef<std::path::Path>) -> Result<Self> {
        let store = object_store::local::LocalFileSystem::new_with_prefix(root)
            .map_err(adapter::backend_error)?;
        Ok(Self {
            store,
            write_lock: futures::lock::Mutex::new(()),
        })
    }
}

#[async_trait::async_trait]
impl StorageProvider for FsProvider {
    async fn get(&self, key: &ObjectKey) -> Result<GetResult> {
        adapter::get(&self.store, key).await
    }

    async fn get_range(&self, key: &ObjectKey, range: std::ops::Range<u64>) -> Result<GetResult> {
        // Reads are not serialized against writes, so the result is consistent with the returned
        // bytes only under the single-writer model (see the module docs).
        adapter::get_range(&self.store, key, range).await
    }

    async fn head(&self, key: &ObjectKey) -> Result<ObjectMeta> {
        adapter::head(&self.store, key).await
    }

    async fn put(&self, key: &ObjectKey, data: Bytes, options: PutOptions) -> Result<PutResult> {
        // LocalFileSystem does not implement conditional Update, so emulate compare-and-swap under
        // the write lock — held across the read-then-write so the check is atomic (see module docs).
        let _guard = self.write_lock.lock().await;
        adapter::put_emulated(&self.store, key, data, options).await
    }

    async fn delete(&self, key: &ObjectKey) -> Result<()> {
        // Serialize with `put` so the emulated compare-and-swap above stays atomic.
        let _guard = self.write_lock.lock().await;
        adapter::delete(&self.store, key).await
    }

    fn list(&self, prefix: &str) -> BoxStream<'_, Result<ObjectMeta>> {
        adapter::list(&self.store, prefix)
    }
}

#[async_trait::async_trait]
impl FromUrl for FsProvider {
    const SCHEME: &'static str = "file";

    /// `file://<dir>`. Creates the root directory on first use (mkdir -p) so the connection string
    /// just works; the parsed tail is the directory path.
    async fn from_url(rest: &str) -> Result<Self> {
        std::fs::create_dir_all(rest).map_err(|err| StorageError::Backend {
            source: Box::new(err),
        })?;
        Self::new(rest)
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
