//! The document store: [`Loami`] and [`Collection`].

use std::sync::Arc;

use bytes::Bytes;
use futures::TryStreamExt;
use loami_storage::{validate_segment, ObjectKey, PutOptions, StorageError, StorageProvider};
use serde_json::Value;

use crate::{DocId, Document, Error, Registry, Result};

/// A handle to a Loami document store, backed by a storage provider.
///
/// The engine is generic over the backend: it talks only to a
/// [`StorageProvider`](loami_storage::StorageProvider), so the same code runs unchanged on the
/// in-memory, filesystem, or Azure provider. Cheap to clone (it is an `Arc` inside).
///
/// ```
/// use std::sync::Arc;
/// use loami::Loami;
/// use loami_storage_memory::MemoryProvider;
/// use serde_json::json;
///
/// # async fn run() -> loami::Result<()> {
/// let db = Loami::open(Arc::new(MemoryProvider::new()));
/// let tasks = db.collection("tasks")?;
/// let id = tasks.insert(json!({ "title": "buy milk", "done": false })).await?;
/// assert_eq!(tasks.get(&id).await?.unwrap()["title"], json!("buy milk"));
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Loami {
    provider: Arc<dyn StorageProvider>,
}

impl Loami {
    /// Opens a store, choosing the backend from a connection string by resolving its scheme through
    /// the [default provider registry](Registry). A scheme is available exactly when a provider is
    /// registered for it: only `mem://` (in-memory) is registered by default. Register any other
    /// provider — a filesystem or cloud backend, or your own — with a [`Registry`] and
    /// [`connect_with`](Self::connect_with). The same program runs across environments by changing
    /// only the URL. For a backend you'd rather build directly, use [`open`](Self::open).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Url`] for a malformed string, [`Error::UnknownScheme`] for a scheme no
    /// registered provider handles, or a storage error if the provider cannot be constructed.
    pub async fn connect(url: &str) -> Result<Self> {
        Self::connect_with(&Registry::default(), url).await
    }

    /// Like [`connect`](Self::connect), but resolves the scheme through `registry` — so you can
    /// register custom providers (or restrict which are available) before connecting.
    ///
    /// # Errors
    ///
    /// As [`connect`](Self::connect).
    pub async fn connect_with(registry: &Registry, url: &str) -> Result<Self> {
        Ok(Self::open(registry.resolve(url).await?))
    }

    /// Opens a store over an existing storage provider.
    #[must_use]
    pub fn open(provider: Arc<dyn StorageProvider>) -> Self {
        Self { provider }
    }

    /// Returns a handle to the named collection.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] unless `name` is a single valid path segment: non-empty,
    /// neither `.` nor `..`, and containing only `[A-Za-z0-9._-]`.
    pub fn collection(&self, name: &str) -> Result<Collection> {
        validate_name(name)?;
        Ok(Collection {
            provider: self.provider.clone(),
            name: name.to_owned(),
        })
    }
}

/// A handle to a collection of JSON documents.
///
/// Each document is stored as one object under the key `"<collection>/<id>"`.
#[derive(Clone)]
pub struct Collection {
    provider: Arc<dyn StorageProvider>,
    name: String,
}

impl Collection {
    /// Inserts `value` as a new document, returning its generated id.
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be serialized or the write fails.
    pub async fn insert(&self, value: Value) -> Result<DocId> {
        let id = DocId::generate();
        let data = serde_json::to_vec(&value)?;
        self.provider
            .put(&self.key(&id), Bytes::from(data), PutOptions::create())
            .await?;
        Ok(id)
    }

    /// Fetches the document with the given id, or `None` if it does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the read fails or the stored bytes are not valid JSON.
    pub async fn get(&self, id: &DocId) -> Result<Option<Value>> {
        match self.provider.get(&self.key(id)).await {
            Ok(result) => Ok(Some(serde_json::from_slice(&result.data)?)),
            Err(StorageError::NotFound { .. }) => Ok(None),
            Err(other) => Err(other.into()),
        }
    }

    /// Replaces the document at `id` with `value` (creating it if absent).
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be serialized or the write fails.
    pub async fn update(&self, id: &DocId, value: Value) -> Result<()> {
        let data = serde_json::to_vec(&value)?;
        self.provider
            .put(&self.key(id), Bytes::from(data), PutOptions::overwrite())
            .await?;
        Ok(())
    }

    /// Deletes the document at `id`. Deleting a missing document succeeds (idempotent).
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    pub async fn delete(&self, id: &DocId) -> Result<()> {
        self.provider.delete(&self.key(id)).await?;
        Ok(())
    }

    /// Returns every document whose fields match `filter` — a JSON object compared by field
    /// equality (`find(json!({}))` returns all documents). This is a full scan; secondary indexes
    /// are a later addition.
    ///
    /// # Errors
    ///
    /// Returns an error if a read fails or a stored document is not valid JSON.
    pub async fn find(&self, filter: Value) -> Result<Vec<Document>> {
        let prefix = format!("{}/", self.name);
        let metas = self.provider.list(&prefix).try_collect::<Vec<_>>().await?;
        let mut matches = Vec::new();
        for meta in metas {
            let result = match self.provider.get(&meta.key).await {
                Ok(result) => result,
                // A document deleted between the list and the read is simply skipped.
                Err(StorageError::NotFound { .. }) => continue,
                Err(other) => return Err(other.into()),
            };
            let value: Value = serde_json::from_slice(&result.data)?;
            if matches_filter(&value, &filter) {
                matches.push(Document {
                    id: id_from_key(meta.key.as_str(), &self.name),
                    value,
                });
            }
        }
        Ok(matches)
    }

    fn key(&self, id: &DocId) -> ObjectKey {
        ObjectKey::new(format!("{}/{}", self.name, id.as_str()))
    }
}

/// A collection name must be a single valid object-key segment, so a document's key
/// (`"<collection>/<id>"`) is always well-formed.
fn validate_name(name: &str) -> Result<()> {
    validate_segment(name).map_err(|reason| Error::InvalidName {
        name: name.to_owned(),
        reason,
    })
}

/// Returns whether `value` matches `filter`. An object filter matches when, for every field it
/// contains, the document has an equal value for that field. A non-object filter matches everything.
fn matches_filter(value: &Value, filter: &Value) -> bool {
    match filter {
        Value::Object(fields) => fields
            .iter()
            .all(|(field, expected)| value.get(field) == Some(expected)),
        _ => true,
    }
}

fn id_from_key(key: &str, collection: &str) -> DocId {
    let prefix = format!("{collection}/");
    DocId::new(key.strip_prefix(&prefix).unwrap_or(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::{self, BoxStream};
    use futures::StreamExt;
    use loami_storage::{key_matches_prefix, Etag, GetResult, ObjectMeta, PutMode, PutResult};
    use loami_storage_memory::MemoryProvider;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // A minimal in-test `StorageProvider`, separate from the real providers, so the engine's tests
    // depend on no provider crate beyond the in-memory default. Objects live in a shared map; only
    // the operations the engine uses (get / put / delete / list) are implemented.
    #[derive(Default)]
    struct TestProvider {
        objects: Arc<Mutex<HashMap<ObjectKey, Bytes>>>,
    }

    #[async_trait::async_trait]
    impl StorageProvider for TestProvider {
        async fn get(&self, key: &ObjectKey) -> loami_storage::Result<GetResult> {
            let data = self
                .objects
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .ok_or_else(|| StorageError::NotFound { key: key.clone() })?;
            let meta = ObjectMeta {
                key: key.clone(),
                size: data.len() as u64,
                etag: Etag::new("0"),
                last_modified: None,
            };
            Ok(GetResult { data, meta })
        }

        async fn get_range(
            &self,
            _key: &ObjectKey,
            _range: std::ops::Range<u64>,
        ) -> loami_storage::Result<GetResult> {
            unimplemented!("test provider does not implement get_range")
        }

        async fn head(&self, _key: &ObjectKey) -> loami_storage::Result<ObjectMeta> {
            unimplemented!("test provider does not implement head")
        }

        async fn put(
            &self,
            key: &ObjectKey,
            data: Bytes,
            options: PutOptions,
        ) -> loami_storage::Result<PutResult> {
            let mut objects = self.objects.lock().unwrap();
            if matches!(options.mode, PutMode::Create) && objects.contains_key(key) {
                return Err(StorageError::AlreadyExists { key: key.clone() });
            }
            objects.insert(key.clone(), data);
            Ok(PutResult {
                etag: Etag::new("0"),
            })
        }

        async fn delete(&self, key: &ObjectKey) -> loami_storage::Result<()> {
            self.objects.lock().unwrap().remove(key);
            Ok(())
        }

        fn list(&self, prefix: &str) -> BoxStream<'_, loami_storage::Result<ObjectMeta>> {
            let metas: Vec<loami_storage::Result<ObjectMeta>> = self
                .objects
                .lock()
                .unwrap()
                .iter()
                .filter(|(key, _)| key_matches_prefix(key.as_str(), prefix))
                .map(|(key, data)| {
                    Ok(ObjectMeta {
                        key: key.clone(),
                        size: data.len() as u64,
                        etag: Etag::new("0"),
                        last_modified: None,
                    })
                })
                .collect();
            stream::iter(metas).boxed()
        }
    }

    async fn exercise(db: Loami) {
        let tasks = db.collection("tasks").unwrap();

        let buy = tasks
            .insert(json!({ "title": "buy milk", "done": false }))
            .await
            .unwrap();
        let ship = tasks
            .insert(json!({ "title": "ship loami", "done": false }))
            .await
            .unwrap();

        // get by id
        assert_eq!(
            tasks.get(&buy).await.unwrap(),
            Some(json!({ "title": "buy milk", "done": false }))
        );
        assert_eq!(tasks.get(&DocId::new("missing")).await.unwrap(), None);

        // find by field equality
        let pending = tasks.find(json!({ "done": false })).await.unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(tasks.find(json!({})).await.unwrap().len(), 2);
        assert!(tasks
            .find(json!({ "title": "nope" }))
            .await
            .unwrap()
            .is_empty());

        // update
        tasks
            .update(&buy, json!({ "title": "buy milk", "done": true }))
            .await
            .unwrap();
        assert_eq!(tasks.get(&buy).await.unwrap().unwrap()["done"], json!(true));
        let still_pending = tasks.find(json!({ "done": false })).await.unwrap();
        assert_eq!(still_pending.len(), 1);
        assert_eq!(still_pending[0].id, ship);

        // delete (idempotent)
        tasks.delete(&buy).await.unwrap();
        tasks.delete(&buy).await.unwrap();
        assert_eq!(tasks.get(&buy).await.unwrap(), None);
    }

    #[tokio::test]
    async fn document_store_on_memory() {
        exercise(Loami::open(Arc::new(MemoryProvider::new()))).await;
    }

    #[tokio::test]
    async fn document_store_on_test_provider() {
        // The same engine drives a second, independent provider — proving it is backend-agnostic.
        exercise(Loami::open(Arc::new(TestProvider::default()))).await;
    }

    #[test]
    fn rejects_invalid_collection_names() {
        let db = Loami::open(Arc::new(MemoryProvider::new()));
        for bad in ["", "a/b", "..", "has space"] {
            assert!(db.collection(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[tokio::test]
    async fn connect_mem() {
        let db = Loami::connect("mem://").await.unwrap();
        let tasks = db.collection("tasks").unwrap();
        let id = tasks.insert(json!({ "x": 1 })).await.unwrap();
        assert!(tasks.get(&id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn connect_requires_provider_registration() {
        // The default registry knows only `mem`; any other scheme is rejected until registered.
        assert!(matches!(
            Loami::connect("test://x").await,
            Err(Error::UnknownScheme { .. })
        ));

        // Once the application registers a provider for the scheme, the same URL resolves and works.
        let mut registry = Registry::default();
        registry.register("test", |_rest| {
            Box::pin(async {
                let provider: Arc<dyn StorageProvider> = Arc::new(TestProvider::default());
                Ok(provider)
            })
        });

        let db = Loami::connect_with(&registry, "test://x").await.unwrap();
        let id = db
            .collection("tasks")
            .unwrap()
            .insert(json!({ "x": 1 }))
            .await
            .unwrap();
        assert!(db
            .collection("tasks")
            .unwrap()
            .get(&id)
            .await
            .unwrap()
            .is_some());
    }

    // `add::<P>()` registers a provider that implements `FromUrl` by type, using its `SCHEME`.
    #[async_trait::async_trait]
    impl loami_storage::FromUrl for TestProvider {
        const SCHEME: &'static str = "test";
        async fn from_url(_rest: &str) -> std::result::Result<Self, loami_storage::StorageError> {
            Ok(Self::default())
        }
    }

    #[tokio::test]
    async fn add_registers_provider_by_type() {
        let mut registry = Registry::empty();
        registry.add::<TestProvider>(); // scheme comes from FromUrl::SCHEME — no closure
        assert_eq!(registry.schemes(), ["test"]);

        let db = Loami::connect_with(&registry, "test://x").await.unwrap();
        let id = db
            .collection("tasks")
            .unwrap()
            .insert(json!({ "x": 1 }))
            .await
            .unwrap();
        assert!(db
            .collection("tasks")
            .unwrap()
            .get(&id)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn connect_rejects_bad_urls() {
        assert!(Loami::connect("nope").await.is_err()); // no scheme separator
        assert!(Loami::connect("ftp://x").await.is_err()); // unregistered scheme
    }

    #[tokio::test]
    async fn unknown_scheme_reports_registered() {
        // A registry with only a custom scheme — the built-ins are absent.
        let mut registry = Registry::empty();
        registry.register("test", |_rest| {
            Box::pin(async {
                let provider: Arc<dyn StorageProvider> = Arc::new(TestProvider::default());
                Ok(provider)
            })
        });

        // An unregistered scheme reports what is registered.
        let err = Loami::connect_with(&registry, "mem://")
            .await
            .err()
            .expect("mem:// is not registered in this registry");
        match err {
            Error::UnknownScheme { registered, .. } => assert_eq!(registered, "test"),
            other => panic!("expected UnknownScheme, got {other:?}"),
        }
    }
}
