//! # loami-storage-azure
//!
//! An Azure Blob Storage [`StorageProvider`] (`azure://`), built on `object_store`'s
//! `MicrosoftAzure`. Unlike the local filesystem, Azure Blob supports conditional `Update`
//! (compare-and-swap by ETag) and lazy listing natively, so this provider maps straight onto the
//! contract with no emulation.
//!
//! The target container must already exist; the provider does not create it.
//!
//! ## Authentication
//!
//! Credentials use the standard Azure conventions, delegated entirely to `object_store`'s
//! [`MicrosoftAzureBuilder`] — nothing Loami-specific. The simple path reads the usual
//! `AZURE_STORAGE_*` environment variables (account key, SAS, service principal, managed identity),
//! the same ones the Azure SDK and CLI use:
//!
//! ```no_run
//! # use loami_storage_azure::AzureProvider;
//! let store = AzureProvider::from_env("my-container")?;
//! # Ok::<(), loami_storage::StorageError>(())
//! ```
//!
//! For full control, hand in a configured [`MicrosoftAzureBuilder`] (re-exported here):
//!
//! ```no_run
//! # use loami_storage_azure::{AzureProvider, MicrosoftAzureBuilder};
//! let store = AzureProvider::from_builder(
//!     MicrosoftAzureBuilder::new()
//!         .with_account("account")
//!         .with_access_key("key")
//!         .with_container_name("my-container"),
//! )?;
//! # Ok::<(), loami_storage::StorageError>(())
//! ```

use bytes::Bytes;
use futures::stream::BoxStream;
use futures::StreamExt;
use loami_storage::{
    Etag, GetResult, ObjectKey, ObjectMeta, PutMode, PutOptions, PutResult, Result, StorageError,
    StorageProvider,
};
use object_store::{path::Path, ObjectStore, ObjectStoreExt};

/// `object_store`'s Azure builder, re-exported so callers configure auth and endpoints through its
/// standard, well-documented surface without depending on `object_store` directly.
pub use object_store::azure::MicrosoftAzureBuilder;

/// A [`StorageProvider`] backed by Azure Blob Storage.
#[derive(Debug)]
pub struct AzureProvider {
    store: object_store::azure::MicrosoftAzure,
}

impl AzureProvider {
    /// Builds a provider for `container`, taking credentials from the standard Azure environment
    /// variables (`AZURE_STORAGE_*`) — account key, SAS, service principal, or managed identity.
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError::Backend`] if the environment does not yield a usable configuration.
    pub fn from_env(container: impl Into<String>) -> Result<Self> {
        Self::from_builder(MicrosoftAzureBuilder::from_env().with_container_name(container))
    }

    /// Builds a provider from a fully-configured [`MicrosoftAzureBuilder`], for any authentication
    /// or endpoint `object_store` supports (including the Azurite emulator).
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError::Backend`] if the builder cannot produce a store.
    pub fn from_builder(builder: MicrosoftAzureBuilder) -> Result<Self> {
        Ok(Self {
            store: builder.build().map_err(backend)?,
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

#[async_trait::async_trait]
impl StorageProvider for AzureProvider {
    async fn get(&self, key: &ObjectKey) -> Result<GetResult> {
        key.validate()?;
        let path = Path::from(key.as_str());
        let result = self.store.get(&path).await.map_err(|e| map_err(key, e))?;
        let meta = to_meta(&result.meta)?;
        let data = result.bytes().await.map_err(|e| map_err(key, e))?;
        Ok(GetResult { data, meta })
    }

    async fn get_range(&self, key: &ObjectKey, range: std::ops::Range<u64>) -> Result<GetResult> {
        key.validate()?;
        let path = Path::from(key.as_str());
        // Validate bounds against the object's size first; object_store does not guarantee a uniform
        // error for an out-of-bounds range. The head also supplies the result's metadata.
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
                meta: to_meta(&head)?,
            });
        }
        let data = self
            .store
            .get_range(&path, range)
            .await
            .map_err(|e| map_err(key, e))?;
        Ok(GetResult {
            data,
            meta: to_meta(&head)?,
        })
    }

    async fn head(&self, key: &ObjectKey) -> Result<ObjectMeta> {
        key.validate()?;
        let path = Path::from(key.as_str());
        let meta = self.store.head(&path).await.map_err(|e| map_err(key, e))?;
        to_meta(&meta)
    }

    async fn put(&self, key: &ObjectKey, data: Bytes, options: PutOptions) -> Result<PutResult> {
        key.validate()?;
        let path = Path::from(key.as_str());
        // Azure Blob implements all three modes natively, including conditional Update (CAS).
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
        let result = self
            .store
            .put_opts(&path, data.into(), mode.into())
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
        match self.store.delete(&path).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(map_err(key, e)),
        }
    }

    fn list(&self, prefix: &str) -> BoxStream<'_, Result<ObjectMeta>> {
        let prefix_path = Path::from(prefix);
        self.store
            .list(Some(&prefix_path))
            .map(|res| to_meta(&res.map_err(backend)?))
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the full conformance suite against Azurite. Ignored by default because it needs the
    /// emulator running with the container pre-created; the `azure` CI job runs it via
    /// `cargo test -p loami-storage-azure -- --ignored`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires Azurite (see the azure CI job)"]
    async fn passes_storage_conformance_suite() {
        let provider = AzureProvider::from_builder(
            MicrosoftAzureBuilder::new()
                .with_use_emulator(true)
                .with_allow_http(true)
                .with_container_name("loami-conformance"),
        )
        .expect("build emulator store");
        loami_storage::conformance::run_conformance_suite(&provider).await;
    }
}
