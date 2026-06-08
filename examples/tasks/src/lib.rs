//! Getting-started example for Loami: a tiny tasks CRUD store over schemaless JSON documents.
//!
//! The same code runs against any backend — only the connection string changes. The binary reads it
//! from `LOAMI_URL` (default [`DEFAULT_URL`]); the guide's getting-started page walks through running
//! it on `mem://` (CI), `file://` (local dev), and `azure://` (production).

use std::sync::Arc;

use loami::{Loami, Registry, Result, StorageError};
use loami_storage::StorageProvider;
use loami_storage_fs::FsProvider;
use serde_json::json;

/// The connection string used when `LOAMI_URL` is unset: in-memory, zero setup.
pub const DEFAULT_URL: &str = "mem://";

/// Builds the provider registry this example supports.
///
/// The engine only knows `mem://`; an application registers the backends it ships. Here that is the
/// filesystem (`file://<dir>`) and — with the `azure` feature — Azure Blob (`azure://<container>`).
#[must_use]
pub fn registry() -> Registry {
    let mut registry = Registry::default(); // mem:// is built in.

    // `file://<dir>` — local-dev persistence. Create the directory on first use so the example just
    // works; an application might instead require it to exist.
    registry.register("file", |dir| {
        std::fs::create_dir_all(dir).map_err(|err| StorageError::Backend {
            source: Box::new(err),
        })?;
        let provider: Arc<dyn StorageProvider> = Arc::new(FsProvider::new(dir)?);
        Ok(provider)
    });

    // `azure://<container>` — credentials come from the standard `AZURE_STORAGE_*` environment.
    #[cfg(feature = "azure")]
    registry.register("azure", |container| {
        let provider: Arc<dyn StorageProvider> =
            Arc::new(loami_storage_azure::AzureProvider::from_env(container)?);
        Ok(provider)
    });

    registry
}

/// Opens a store at `url` using this example's [`registry`], so `file://` (and, with the `azure`
/// feature, `azure://`) resolve in addition to the built-in `mem://`.
///
/// # Errors
///
/// Returns an error if the URL's scheme is not registered or the provider cannot be constructed.
pub fn connect(url: &str) -> Result<Loami> {
    Loami::connect_with(&registry(), url)
}

/// Runs the tasks walkthrough against an open store, printing each step. This doubles as the
/// example's executable API spec: insert → find → update → get → delete.
///
/// # Errors
///
/// Returns an error if any storage operation fails or a stored document is not valid JSON.
pub async fn run(db: &Loami) -> Result<()> {
    let tasks = db.collection("tasks")?;

    // On a persistent backend (`file://`, `azure://`) this grows across runs; on `mem://` it is
    // always empty — the same code, different durability.
    let existing = tasks.find(json!({})).await?;
    if !existing.is_empty() {
        println!("(found {} task(s) left by a previous run)", existing.len());
    }

    // Insert: schemaless JSON in, a generated id out.
    let buy = tasks
        .insert(json!({ "title": "buy milk", "done": false }))
        .await?;
    let ship = tasks
        .insert(json!({ "title": "ship loami", "done": false }))
        .await?;
    println!("inserted 2 tasks (e.g. {buy})");

    // Find: a field-equality query over the collection.
    let pending = tasks.find(json!({ "done": false })).await?;
    println!("{} task(s) pending", pending.len());

    // Update: replace a document by id.
    tasks
        .update(&buy, json!({ "title": "buy milk", "done": true }))
        .await?;

    // Get: fetch one document by id.
    if let Some(task) = tasks.get(&buy).await? {
        println!("task {buy} -> done={}", task["done"]);
    }

    // Delete: removing a document is idempotent.
    tasks.delete(&ship).await?;
    println!("deleted task {ship}");

    Ok(())
}
