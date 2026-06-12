# Document store

Loami stores schemaless JSON documents in collections, over any storage backend. Open a store with a
provider, then insert, get, update, find, and delete documents:

```rust
use loami::Loami;
use serde_json::json;

let db = Loami::connect("mem://").await?;
let tasks = db.collection("tasks")?;

let id = tasks.insert(json!({ "title": "buy milk", "done": false })).await?; // -> DocId
let pending = tasks.find(json!({ "done": false })).await?;                    // field-equality query
tasks.update(&id, json!({ "title": "buy milk", "done": true })).await?;
let task = tasks.get(&id).await?;                                             // Option<Value>
tasks.delete(&id).await?;
```

## Connecting

`Loami::connect(url)` resolves the URL's scheme through a provider registry, so the same program runs
across environments by changing only the URL. Only `mem://` is registered by default — the engine is
agnostic about every other backend, which the application registers:

```rust
use loami::{Loami, Registry};
use loami_storage_fs::FsProvider;
use std::sync::Arc;

// mem:// works out of the box (CI, tests).
let db = Loami::connect("mem://").await?;

// Register a provider to add its scheme, then switch environments by URL alone.
let mut registry = Registry::default();
registry.register("file", |path| {
    let path = path.to_owned();
    Box::pin(async move { Ok(Arc::new(FsProvider::new(&path)?) as _) })
});
let db = Loami::connect_with(&registry, "file://./data").await?;   // local dev — persists on disk
```

Any [storage provider](./storage.md) registers the same way — add its crate and register its scheme.
For a backend you'd rather build directly, call `Loami::open(provider)`.

## Model

- A **collection** is a named group of documents; its name must be a single `[A-Za-z0-9._-]` segment.
- Each **document** is arbitrary JSON, identified by a generated `DocId`, and stored as one object
  under the key `"<collection>/<id>"`.
- **`find`** takes a JSON object and returns the documents whose fields all equal it — `find(json!({}))`
  returns everything. It is a full scan today; secondary indexes are planned.
- **`update`** replaces the document at an id (creating it if absent); **`delete`** is idempotent.
