# Getting started

The quickest way to see Loami is the **tasks example** — a small CRUD store over schemaless JSON
documents that runs identically across every backend. It lives in
[`examples/tasks`](https://github.com/waviisoft/loami/tree/main/examples/tasks) and doubles as the
engine's executable API spec.

## The code

```rust
use loami::Loami;
use serde_json::json;

# async fn walkthrough(db: &Loami) -> loami::Result<()> {
let tasks = db.collection("tasks")?;

let buy = tasks.insert(json!({ "title": "buy milk", "done": false })).await?; // -> DocId
let pending = tasks.find(json!({ "done": false })).await?;                    // field-equality query
tasks.update(&buy, json!({ "title": "buy milk", "done": true })).await?;
let task = tasks.get(&buy).await?;                                            // Option<Value>
tasks.delete(&buy).await?;
# Ok(())
# }
```

## One program, three backends

The backend is chosen by the `LOAMI_URL` environment variable; nothing else changes:

| Context | `LOAMI_URL` | Notes |
| --- | --- | --- |
| **CI / tests** | `mem://` | default; in-memory, zero setup, ephemeral |
| **Local dev** | `file://./loami-data` | persists on disk between runs, inspectable |
| **Production** | `azure://<container>` | Azure Blob; standard `AZURE_STORAGE_*` auth (needs the `azure` feature) |

The engine only knows `mem://`. The example registers `file://` (and, behind the `azure` feature,
`azure://`) in its
[`registry()`](https://github.com/waviisoft/loami/blob/main/examples/tasks/src/lib.rs) — which is how
an application declares the backends it ships. See [Connecting](./document-store.md#connecting) for the
registry mechanism and [Storage providers](./storage.md) for the available backends.

## Running it

```sh
# in-memory (default) — nothing to set up
cargo run -p loami-example-tasks

# local filesystem — persists in ./loami-data, inspectable on disk
LOAMI_URL=file://./loami-data cargo run -p loami-example-tasks

# Azure Blob — needs the feature and the standard AZURE_STORAGE_* credentials
LOAMI_URL=azure://my-container cargo run -p loami-example-tasks --features azure
```

Run the `file://` command twice and the second run reports the tasks the first one left behind — the
same code, now durable.

## What proves it keeps working

The example's smoke tests run the full walkthrough on `mem://` and `file://` on every build, and one
test writes through a `file://` connection and reads it back through a fresh one — so the
dev/prod-parity guarantee (same code, swap the URL) is verified continuously, with zero services.
