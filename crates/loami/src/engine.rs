//! The document store: [`Loami`] and [`Collection`].

use std::sync::Arc;

use bytes::Bytes;
use futures::TryStreamExt;
use loami_storage::{ObjectKey, PutOptions, StorageError, StorageProvider};
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
    pub fn connect(url: &str) -> Result<Self> {
        Self::connect_with(&Registry::default(), url)
    }

    /// Like [`connect`](Self::connect), but resolves the scheme through `registry` — so you can
    /// register custom providers (or restrict which are available) before connecting.
    ///
    /// # Errors
    ///
    /// As [`connect`](Self::connect).
    pub fn connect_with(registry: &Registry, url: &str) -> Result<Self> {
        Ok(Self::open(registry.resolve(url)?))
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
    /// Returns [`Error::InvalidName`] if `name` is empty or contains characters outside
    /// `[A-Za-z0-9._-]`.
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

/// A collection name must be a single, non-empty object-key segment.
fn validate_name(name: &str) -> Result<()> {
    let invalid = |reason| Error::InvalidName {
        name: name.to_owned(),
        reason,
    };
    if name.is_empty() {
        return Err(invalid("collection name must not be empty"));
    }
    if name == "." || name == ".." {
        return Err(invalid("collection name must not be '.' or '..'"));
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(invalid("collection name may contain only [A-Za-z0-9._-]"));
    }
    Ok(())
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
    use loami_storage_fs::FsProvider;
    use loami_storage_memory::MemoryProvider;
    use serde_json::json;

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
    async fn document_store_on_filesystem() {
        let dir = tempfile::tempdir().unwrap();
        exercise(Loami::open(Arc::new(FsProvider::new(dir.path()).unwrap()))).await;
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
        let db = Loami::connect("mem://").unwrap();
        let tasks = db.collection("tasks").unwrap();
        let id = tasks.insert(json!({ "x": 1 })).await.unwrap();
        assert!(tasks.get(&id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn connect_file_requires_registration_then_works() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("file://{}", dir.path().display());

        // `file` is not in the default registry — the engine is agnostic about it.
        assert!(matches!(
            Loami::connect(&url),
            Err(Error::UnknownScheme { .. })
        ));

        // Once the application registers a filesystem provider, the same URL works and persists.
        let mut registry = Registry::default();
        registry.register("file", |path| {
            let provider: Arc<dyn StorageProvider> = Arc::new(FsProvider::new(path)?);
            Ok(provider)
        });

        let id = Loami::connect_with(&registry, &url)
            .unwrap()
            .collection("tasks")
            .unwrap()
            .insert(json!({ "x": 1 }))
            .await
            .unwrap();

        // A fresh connection to the same directory sees the data — the local-dev story.
        let reopened = Loami::connect_with(&registry, &url).unwrap();
        assert!(reopened
            .collection("tasks")
            .unwrap()
            .get(&id)
            .await
            .unwrap()
            .is_some());
    }

    #[test]
    fn connect_rejects_bad_urls() {
        assert!(Loami::connect("nope").is_err()); // no scheme separator
        assert!(Loami::connect("ftp://x").is_err()); // unregistered scheme
    }

    #[tokio::test]
    async fn connect_with_custom_registry() {
        // A registry with only a custom scheme — the built-ins are absent.
        let mut registry = Registry::empty();
        registry.register("ram", |_rest| {
            let provider: Arc<dyn StorageProvider> = Arc::new(MemoryProvider::new());
            Ok(provider)
        });

        let db = Loami::connect_with(&registry, "ram://").unwrap();
        let id = db
            .collection("t")
            .unwrap()
            .insert(json!({ "x": 1 }))
            .await
            .unwrap();
        assert!(db
            .collection("t")
            .unwrap()
            .get(&id)
            .await
            .unwrap()
            .is_some());

        // An unregistered scheme reports what is registered.
        let err = Loami::connect_with(&registry, "mem://")
            .err()
            .expect("mem:// is not registered in this registry");
        match err {
            Error::UnknownScheme { registered, .. } => assert_eq!(registered, "ram"),
            other => panic!("expected UnknownScheme, got {other:?}"),
        }
    }
}
