# Document store

Loami stores schemaless JSON documents in collections, over any storage backend. Open a store with a
provider, then insert, get, update, find, and delete documents:

```rust
use loami::Loami;
use serde_json::json;

let db = Loami::connect("mem://")?;
let tasks = db.collection("tasks")?;

let id = tasks.insert(json!({ "title": "buy milk", "done": false })).await?; // -> DocId
let pending = tasks.find(json!({ "done": false })).await?;                    // field-equality query
tasks.update(&id, json!({ "title": "buy milk", "done": true })).await?;
let task = tasks.get(&id).await?;                                             // Option<Value>
tasks.delete(&id).await?;
```

## Connecting

`Loami::connect(url)` resolves the URL's scheme through a provider registry, so the same program runs
across environments by changing only the URL:

```rust
let db = Loami::connect("mem://")?;          // CI / tests — ephemeral, zero setup
let db = Loami::connect("file://./data")?;   // local dev — persists on disk
```

A scheme is available exactly when a provider is registered for it:

- **Built-in** (always available): `mem://` and `file://`.
- **Officially-supported** (optional, enabled by a Cargo feature): for example the `azure` feature
  registers `azure://<container>` (Azure Blob, using the standard `AZURE_STORAGE_*` credentials).
- **Custom**: register your own with a `Registry` and `Loami::connect_with`.

For a backend you'd rather build directly, construct a provider and call `Loami::open(provider)`.

## Model

- A **collection** is a named group of documents; its name must be a single `[A-Za-z0-9._-]` segment.
- Each **document** is arbitrary JSON, identified by a generated `DocId`, and stored as one object
  under the key `"<collection>/<id>"`.
- **`find`** takes a JSON object and returns the documents whose fields all equal it — `find(json!({}))`
  returns everything. It is a full scan today; secondary indexes are planned.
- **`update`** replaces the document at an id (creating it if absent); **`delete`** is idempotent.
