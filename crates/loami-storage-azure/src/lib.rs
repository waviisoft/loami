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
use loami_storage::{
    FromUrl, GetResult, ObjectKey, ObjectMeta, PutOptions, PutResult, Result, StorageProvider,
};
use loami_storage_object_store as adapter;

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
    /// Returns a [`StorageError::Backend`](loami_storage::StorageError::Backend) if the environment
    /// does not yield a usable configuration.
    pub fn from_env(container: impl Into<String>) -> Result<Self> {
        Self::from_builder(MicrosoftAzureBuilder::from_env().with_container_name(container))
    }

    /// Builds a provider from a fully-configured [`MicrosoftAzureBuilder`], for any authentication
    /// or endpoint `object_store` supports (including the Azurite emulator).
    ///
    /// # Errors
    ///
    /// Returns a [`StorageError::Backend`](loami_storage::StorageError::Backend) if the builder
    /// cannot produce a store.
    pub fn from_builder(builder: MicrosoftAzureBuilder) -> Result<Self> {
        Ok(Self {
            store: builder.build().map_err(adapter::backend_error)?,
        })
    }
}

#[async_trait::async_trait]
impl StorageProvider for AzureProvider {
    async fn get(&self, key: &ObjectKey) -> Result<GetResult> {
        adapter::get(&self.store, key).await
    }

    async fn get_range(&self, key: &ObjectKey, range: std::ops::Range<u64>) -> Result<GetResult> {
        adapter::get_range(&self.store, key, range).await
    }

    async fn head(&self, key: &ObjectKey) -> Result<ObjectMeta> {
        adapter::head(&self.store, key).await
    }

    async fn put(&self, key: &ObjectKey, data: Bytes, options: PutOptions) -> Result<PutResult> {
        // Azure Blob implements all three modes natively, including conditional Update (CAS), so no
        // emulation or locking is needed.
        adapter::put_native(&self.store, key, data, options).await
    }

    async fn delete(&self, key: &ObjectKey) -> Result<()> {
        adapter::delete(&self.store, key).await
    }

    fn list(&self, prefix: &str) -> BoxStream<'_, Result<ObjectMeta>> {
        adapter::list(&self.store, prefix)
    }
}

#[async_trait::async_trait]
impl FromUrl for AzureProvider {
    const SCHEME: &'static str = "azure";

    /// `azure://<container>`; credentials come from the standard `AZURE_STORAGE_*` environment.
    async fn from_url(rest: &str) -> Result<Self> {
        Self::from_env(rest)
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
