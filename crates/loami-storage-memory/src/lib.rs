//! # loami-storage-memory
//!
//! An in-memory [`StorageProvider`] backed by a `HashMap`, with monotonically increasing ETags.
//!
//! It holds everything in process memory and is lost when dropped, which makes it ideal for unit
//! tests, CI, and the `mem://` connection scheme. It is also the contract's **reference
//! implementation**: hand-written (rather than wrapping a third-party object store) so the
//! [`conformance`](loami_storage::conformance) suite is validated against an independent backend.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use bytes::Bytes;
use futures::stream::{self, BoxStream};
use futures::StreamExt;
use loami_storage::{
    key_matches_prefix, Etag, FromUrl, GetResult, ObjectKey, ObjectMeta, PutMode, PutOptions,
    PutResult, Result, StorageError, StorageProvider,
};

/// An in-memory object store, optionally capped at a maximum number of stored value bytes.
///
/// Cloneable state is held behind a [`Mutex`]; share it across tasks via `Arc<MemoryProvider>`.
#[derive(Debug, Default)]
pub struct MemoryProvider {
    objects: Mutex<HashMap<ObjectKey, Entry>>,
    next_etag: AtomicU64,
    /// Running total of stored value bytes; mutated only while holding `objects`.
    used_bytes: AtomicUsize,
    /// Optional cap on total stored value bytes; `None` is unbounded.
    max_bytes: Option<usize>,
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
    /// Creates an empty, unbounded in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty in-memory store capped at `max_bytes` of stored value bytes. A write that
    /// would push the total over the cap fails with [`StorageError::QuotaExceeded`].
    #[must_use]
    pub fn with_max_bytes(max_bytes: usize) -> Self {
        Self {
            max_bytes: Some(max_bytes),
            ..Self::default()
        }
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

        // Enforce the optional cap, accounting for replacing an existing value at this key.
        let old_len = objects.get(key).map_or(0, |entry| entry.data.len());
        let new_used = self
            .used_bytes
            .load(Ordering::Relaxed)
            .saturating_sub(old_len)
            + data.len();
        if let Some(max) = self.max_bytes {
            if new_used > max {
                return Err(StorageError::QuotaExceeded {
                    limit: max,
                    attempted: new_used,
                });
            }
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
        self.used_bytes.store(new_used, Ordering::Relaxed);
        Ok(PutResult { etag })
    }

    async fn delete(&self, key: &ObjectKey) -> Result<()> {
        key.validate()?;
        let mut objects = self.objects.lock().expect("lock poisoned");
        if let Some(entry) = objects.remove(key) {
            self.used_bytes
                .fetch_sub(entry.data.len(), Ordering::Relaxed);
        }
        Ok(())
    }

    fn list(&self, prefix: &str) -> BoxStream<'_, Result<ObjectMeta>> {
        // Snapshot the matching entries under the lock, then stream the snapshot; the lock is not
        // held while the stream is consumed. Sorting is incidental — it keeps this in-memory
        // backend's output stable for tests — but the contract leaves list order unspecified, so
        // callers must not rely on it.
        let mut metas: Vec<ObjectMeta> = {
            let objects = self.objects.lock().expect("lock poisoned");
            objects
                .iter()
                .filter(|(key, _)| key_matches_prefix(key.as_str(), prefix))
                .map(|(key, entry)| object_meta(key, entry))
                .collect()
        };
        metas.sort_by(|a, b| a.key.as_str().cmp(b.key.as_str()));
        stream::iter(metas.into_iter().map(Ok::<_, StorageError>)).boxed()
    }
}

#[async_trait::async_trait]
impl FromUrl for MemoryProvider {
    const SCHEME: &'static str = "mem";

    /// A fresh, empty store per connect. An optional `?max_bytes=<n>` query caps it: `mem://` is
    /// unbounded, `mem://?max_bytes=67108864` caps the store at 64 MiB.
    async fn from_url(rest: &str) -> Result<Self> {
        Ok(match parse_max_bytes(rest)? {
            Some(max) => Self::with_max_bytes(max),
            None => Self::new(),
        })
    }
}

/// Extracts `max_bytes` from the `?key=value&...` query in a `mem://` URL tail, if present.
fn parse_max_bytes(rest: &str) -> Result<Option<usize>> {
    let Some((_, query)) = rest.split_once('?') else {
        return Ok(None);
    };
    for pair in query.split('&') {
        if let Some(("max_bytes", value)) = pair.split_once('=') {
            let bytes = value
                .parse::<usize>()
                .map_err(|err| StorageError::Backend {
                    source: format!("invalid mem:// max_bytes {value:?}: {err}").into(),
                })?;
            return Ok(Some(bytes));
        }
    }
    Ok(None)
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

    #[tokio::test]
    async fn enforces_max_bytes_cap() {
        let provider = MemoryProvider::with_max_bytes(10);
        let (a, b) = (ObjectKey::new("a"), ObjectKey::new("b"));

        // Under the cap: ok (5 of 10 bytes used).
        provider
            .put(&a, Bytes::from_static(b"12345"), PutOptions::overwrite())
            .await
            .unwrap();

        // A write that would push the total to 12 > 10 is rejected.
        let err = provider
            .put(&b, Bytes::from_static(b"1234567"), PutOptions::overwrite())
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            StorageError::QuotaExceeded {
                limit: 10,
                attempted: 12
            }
        ));

        // Deleting frees space, so the same write then fits.
        provider.delete(&a).await.unwrap();
        provider
            .put(&b, Bytes::from_static(b"1234567"), PutOptions::overwrite())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn overwrite_adjusts_accounting() {
        let provider = MemoryProvider::with_max_bytes(5);
        let k = ObjectKey::new("k");
        provider
            .put(&k, Bytes::from_static(b"12345"), PutOptions::overwrite())
            .await
            .unwrap();
        // Overwriting with smaller data frees the difference, so a second key then fits.
        provider
            .put(&k, Bytes::from_static(b"1"), PutOptions::overwrite())
            .await
            .unwrap();
        provider
            .put(
                &ObjectKey::new("k2"),
                Bytes::from_static(b"123"),
                PutOptions::overwrite(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn from_url_parses_max_bytes() {
        assert_eq!(MemoryProvider::from_url("").await.unwrap().max_bytes, None);
        assert_eq!(
            MemoryProvider::from_url("?max_bytes=64")
                .await
                .unwrap()
                .max_bytes,
            Some(64)
        );
        assert!(MemoryProvider::from_url("?max_bytes=lots").await.is_err());
    }
}
