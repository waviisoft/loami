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

`Loami::connect(url)` picks the storage backend from a connection string, so the same program runs
across environments by changing only the URL:

```rust
let db = Loami::connect("mem://")?;                // CI / tests — ephemeral, zero setup
let db = Loami::connect("file://./data")?;         // local dev — persists on disk
let db = Loami::connect("azure://my-container")?;  // prod — needs the `azure` feature + AZURE_STORAGE_* env
```

`mem://` and `file://` are built in; `azure://` requires building Loami with the `azure` feature.
The engine talks only to a [storage provider](./storage.md), so backends are pluggable and the set
grows over time — for one not covered by `connect`, construct a provider yourself and call
`Loami::open(provider)`.

## Model

- A **collection** is a named group of documents; its name must be a single `[A-Za-z0-9._-]` segment.
- Each **document** is arbitrary JSON, identified by a generated `DocId`, and stored as one object
  under the key `"<collection>/<id>"`.
- **`find`** takes a JSON object and returns the documents whose fields all equal it — `find(json!({}))`
  returns everything. It is a full scan today; secondary indexes are planned.
- **`update`** replaces the document at an id (creating it if absent); **`delete`** is idempotent.
