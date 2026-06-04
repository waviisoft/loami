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
    Etag, GetResult, ObjectKey, ObjectMeta, PutMode, PutOptions, PutResult, Result, StorageError,
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

fn object_meta(key: &ObjectKey, entry: &Entry) -> ObjectMeta {
    ObjectMeta {
        key: key.clone(),
        size: entry.data.len() as u64,
        etag: entry.etag.clone(),
        last_modified: Some(entry.last_modified),
    }
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
    async fn get(&self, key: &ObjectKey) -> Result<GetResult> {
        key.validate()?;
        let objects = self.objects.lock().expect("lock poisoned");
        let entry = objects
            .get(key)
            .ok_or_else(|| StorageError::NotFound { key: key.clone() })?;
        Ok(GetResult {
            data: entry.data.clone(),
            meta: object_meta(key, entry),
        })
    }

    async fn get_range(&self, key: &ObjectKey, range: std::ops::Range<u64>) -> Result<GetResult> {
        key.validate()?;
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
        Ok(GetResult {
            data: entry.data.slice(range.start as usize..range.end as usize),
            meta: object_meta(key, entry),
        })
    }

    async fn head(&self, key: &ObjectKey) -> Result<ObjectMeta> {
        key.validate()?;
        let objects = self.objects.lock().expect("lock poisoned");
        let entry = objects
            .get(key)
            .ok_or_else(|| StorageError::NotFound { key: key.clone() })?;
        Ok(object_meta(key, entry))
    }

    async fn put(&self, key: &ObjectKey, data: Bytes, options: PutOptions) -> Result<PutResult> {
        key.validate()?;
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
        key.validate()?;
        let mut objects = self.objects.lock().expect("lock poisoned");
        objects.remove(key);
        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>> {
        let objects = self.objects.lock().expect("lock poisoned");
        Ok(objects
            .iter()
            .filter(|(key, _)| key_has_prefix(key.as_str(), prefix))
            .map(|(key, entry)| object_meta(key, entry))
            .collect())
    }
}

/// Path-segment prefix match (directory-style): an empty prefix matches everything, a trailing `/`
/// is ignored, and a non-empty prefix matches only keys strictly beneath it on a `/` boundary.
fn key_has_prefix(key: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }
    let prefix = prefix.strip_suffix('/').unwrap_or(prefix);
    key.len() > prefix.len() && key.starts_with(prefix) && key.as_bytes()[prefix.len()] == b'/'
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
        assert_eq!(
            provider.get(&key).await.unwrap().data,
            Bytes::from_static(b"hi")
        );
    }
}
