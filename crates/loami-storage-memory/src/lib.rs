//! # loami-storage-memory
//!
//! An in-memory [`StorageProvider`] backed by a `HashMap`, with monotonically increasing ETags.
//!
//! It holds everything in process memory and is lost when dropped, which makes it ideal for unit
//! tests, CI, and the `mem://` connection scheme. It is also the contract's **reference
//! implementation**: hand-written (rather than wrapping a third-party object store) so the
//! [`conformance`](loami_storage::conformance) suite is validated against an independent backend.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use bytes::Bytes;
use loami_storage::{
    Etag, ObjectKey, ObjectMeta, PutMode, PutOptions, PutResult, Result, StorageError,
    StorageProvider,
};

/// An in-memory object store.
///
/// Cloneable state is held behind a [`Mutex`]; share it across tasks via `Arc<MemoryProvider>`.
#[derive(Debug, Default)]
pub struct MemoryProvider {
    objects: Mutex<HashMap<ObjectKey, Entry>>,
    next_etag: AtomicU64,
}

#[derive(Clone, Debug)]
struct Entry {
    data: Bytes,
    etag: Etag,
    last_modified: SystemTime,
}

impl MemoryProvider {
    /// Creates an empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn mint_etag(&self) -> Etag {
        Etag::new(self.next_etag.fetch_add(1, Ordering::Relaxed).to_string())
    }
}

#[async_trait::async_trait]
impl StorageProvider for MemoryProvider {
    async fn get(&self, key: &ObjectKey) -> Result<Bytes> {
        let objects = self.objects.lock().expect("lock poisoned");
        objects
            .get(key)
            .map(|entry| entry.data.clone())
            .ok_or_else(|| StorageError::NotFound { key: key.clone() })
    }

    async fn get_range(&self, key: &ObjectKey, range: std::ops::Range<u64>) -> Result<Bytes> {
        let objects = self.objects.lock().expect("lock poisoned");
        let entry = objects
            .get(key)
            .ok_or_else(|| StorageError::NotFound { key: key.clone() })?;
        let size = entry.data.len() as u64;
        if range.start > range.end || range.end > size {
            return Err(StorageError::InvalidRange {
                key: key.clone(),
                start: range.start,
                end: range.end,
                size,
            });
        }
        Ok(entry.data.slice(range.start as usize..range.end as usize))
    }

    async fn head(&self, key: &ObjectKey) -> Result<ObjectMeta> {
        let objects = self.objects.lock().expect("lock poisoned");
        let entry = objects
            .get(key)
            .ok_or_else(|| StorageError::NotFound { key: key.clone() })?;
        Ok(ObjectMeta {
            key: key.clone(),
            size: entry.data.len() as u64,
            etag: entry.etag.clone(),
            last_modified: Some(entry.last_modified),
        })
    }

    async fn put(&self, key: &ObjectKey, data: Bytes, options: PutOptions) -> Result<PutResult> {
        let mut objects = self.objects.lock().expect("lock poisoned");
        match options.mode {
            PutMode::Overwrite => {}
            PutMode::Create => {
                if objects.contains_key(key) {
                    return Err(StorageError::AlreadyExists { key: key.clone() });
                }
            }
            PutMode::Update { expected } => match objects.get(key) {
                Some(entry) if entry.etag == expected => {}
                _ => return Err(StorageError::Precondition { key: key.clone() }),
            },
        }

        let etag = self.mint_etag();
        objects.insert(
            key.clone(),
            Entry {
                data,
                etag: etag.clone(),
                last_modified: SystemTime::now(),
            },
        );
        Ok(PutResult { etag })
    }

    async fn delete(&self, key: &ObjectKey) -> Result<()> {
        let mut objects = self.objects.lock().expect("lock poisoned");
        objects.remove(key);
        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>> {
        let objects = self.objects.lock().expect("lock poisoned");
        Ok(objects
            .iter()
            .filter(|(key, _)| key.as_str().starts_with(prefix))
            .map(|(key, entry)| ObjectMeta {
                key: key.clone(),
                size: entry.data.len() as u64,
                etag: entry.etag.clone(),
                last_modified: Some(entry.last_modified),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn passes_storage_conformance_suite() {
        let provider = MemoryProvider::new();
        loami_storage::conformance::run_conformance_suite(&provider).await;
    }

    #[tokio::test]
    async fn put_then_get_returns_the_same_bytes() {
        let provider = MemoryProvider::new();
        let key = ObjectKey::new("greeting");
        provider
            .put(&key, Bytes::from_static(b"hi"), PutOptions::overwrite())
            .await
            .unwrap();
        assert_eq!(provider.get(&key).await.unwrap(), Bytes::from_static(b"hi"));
    }
}
