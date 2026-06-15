//! Getting-started example for Loami: a tiny tasks CRUD store over schemaless JSON documents.
//!
//! The same code runs against any backend — only the connection string changes. The binary reads it
//! from `LOAMI_URL` (default [`loami::MEM_URL`]); the guide's getting-started page walks through
//! running it on `mem://` (CI), `file://` (local), and — with the `azure` feature — `azure://`
//! (cloud, which is equally usable locally to validate or reproduce a cloud setup).

use loami::{Loami, Registry, Result};
use loami_storage_fs::FsProvider;
use serde_json::json;

/// Builds the provider registry this example supports.
///
/// The engine knows only `mem://`; an application registers, by type, the backends it ships. Here
/// that is the filesystem (`file://<dir>`) and — with the `azure` feature — Azure Blob
/// (`azure://<container>`). Each provider owns its scheme and how it builds from the URL tail.
#[must_use]
pub fn registry() -> Registry {
    let mut registry = Registry::default(); // mem:// is built in.
    registry.add::<FsProvider>();
    #[cfg(feature = "azure")]
    registry.add::<loami_storage_azure::AzureProvider>();
    registry
}

/// Opens a store at `url` using this example's [`registry`], so `file://` (and, with the `azure`
/// feature, `azure://`) resolve in addition to the built-in `mem://`.
///
/// # Errors
///
/// Returns an error if the URL's scheme is not registered or the provider cannot be constructed.
pub async fn connect(url: &str) -> Result<Loami> {
    Loami::connect_with(&registry(), url).await
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
