# Document store

Loami stores schemaless JSON documents in collections, over any storage backend. Open a store with a
provider, then insert, get, update, find, and delete documents:

```rust
use std::sync::Arc;
use loami::Loami;
use loami_storage_memory::MemoryProvider;
use serde_json::json;

let db = Loami::open(Arc::new(MemoryProvider::new()));
let tasks = db.collection("tasks")?;

let id = tasks.insert(json!({ "title": "buy milk", "done": false })).await?; // -> DocId
let pending = tasks.find(json!({ "done": false })).await?;                    // field-equality query
tasks.update(&id, json!({ "title": "buy milk", "done": true })).await?;
let task = tasks.get(&id).await?;                                             // Option<Value>
tasks.delete(&id).await?;
```

## Backends

The engine talks only to a [storage provider](./storage.md), so the *same code* runs on any backend
— swap `MemoryProvider` for any other provider and nothing else changes. Providers are pluggable and
independently versioned (and may live in their own crates), so the set grows over time. (A
`Loami::connect(url)` convenience that picks a provider from a connection string — `mem://`,
`file://…`, and so on — is coming next.)

## Model

- A **collection** is a named group of documents; its name must be a single `[A-Za-z0-9._-]` segment.
- Each **document** is arbitrary JSON, identified by a generated `DocId`, and stored as one object
  under the key `"<collection>/<id>"`.
- **`find`** takes a JSON object and returns the documents whose fields all equal it — `find(json!({}))`
  returns everything. It is a full scan today; secondary indexes are planned.
- **`update`** replaces the document at an id (creating it if absent); **`delete`** is idempotent.
