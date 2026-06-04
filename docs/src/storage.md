# Storage providers

Loami never persists to a specific cloud or filesystem directly. It reads and writes through a
**storage provider** — an implementation of the `StorageProvider` trait from the `loami-storage`
crate. The engine depends only on that trait, so any backend that satisfies the contract is a
drop-in replacement. (This mirrors how Terraform separates its core from its providers.)

## The contract

A provider exposes a small object-store surface:

| Operation | Purpose |
| --- | --- |
| `get` / `get_range` | read a whole object (or a byte range) **plus its metadata, including the ETag** |
| `head` | size + ETag + last-modified, without the body |
| `put` | write, with a conditional mode (below) |
| `delete` | remove an object (idempotent) |
| `list` / `list_all` | **stream** objects under a key prefix (lazy), or collect them into a `Vec` |

### Keys

A key is a non-empty, `/`-separated path whose segments contain only `[A-Za-z0-9._-]` — no leading,
trailing, or empty segments, and no `.`/`..`. Providers reject anything else with an `InvalidKey`
error, so keys round-trip byte-for-byte across every backend and path-traversal segments are barred.
`list(prefix)` matches on segment boundaries (directory-style): `list("a/b")` returns `a/b/c` but not
`a/bc`. List order is unspecified and may differ between backends — sort if you need a stable order.

### Conditional writes (compare-and-swap)

`put` takes a `PutMode` that provides optimistic-concurrency control:

- **`Overwrite`** — unconditional.
- **`Create`** — write only if the key is absent (otherwise `AlreadyExists`).
- **`Update { expected }`** — write only if the current ETag matches `expected` (otherwise
  `Precondition`).

These are the primitives Loami uses for atomic commits, and later for single-writer fencing across a
cluster.

## Built-in providers

- **Memory** (`loami-storage-memory`) — an in-process store for tests, CI, and ephemeral use. It is
  also the contract's reference implementation.
- **Filesystem** (`loami-storage-fs`) — a local-filesystem store rooted at a directory, built on
  `object_store`. For local development and single-node persistence. It emulates the conditional
  `Update` (compare-and-swap) that the local backend lacks, under a single-writer assumption.

More providers (Azure Blob, …) are tracked on the [roadmap](./roadmap.md).

## Using a provider

```rust
use bytes::Bytes;
use loami_storage::{ObjectKey, PutOptions, StorageProvider};
use loami_storage_memory::MemoryProvider;

let store = MemoryProvider::new();
let key = ObjectKey::new("notes/hello");

// Create only if absent; returns the new object's ETag.
store
    .put(&key, Bytes::from_static(b"hi"), PutOptions::create())
    .await?;

// A read returns the bytes *and* the object's metadata in one call — so the ETag belongs to
// exactly the bytes you just read, with no separate `head` and no window for it to drift.
let read = store.get(&key).await?;
assert_eq!(read.data, Bytes::from_static(b"hi"));

// Compare-and-swap using the ETag from that same read.
store
    .put(&key, Bytes::from_static(b"updated"), PutOptions::update(read.meta.etag))
    .await?;
```

## Implementing a provider

Every provider must pass the shared **conformance suite**, which asserts the full contract — including
the CAS semantics above. The suite lives behind the `conformance` feature of `loami-storage`; enable
it as a dev-dependency:

```toml
# Cargo.toml
[dependencies]
loami-storage = "0.0.1"

[dev-dependencies]
loami-storage = { version = "0.0.1", features = ["conformance"] }
tokio = { version = "1", features = ["macros", "rt"] }
```

Listing the crate in both sections lets Cargo enable `conformance` for the test build only — your
provider's release build never pulls the suite in. (Within this workspace the providers use a path
dependency, e.g. `{ path = "../loami-storage", features = ["conformance"] }`.)

Then call it from a test:

```rust
#[tokio::test]
async fn passes_conformance() {
    let provider = MyProvider::new();
    loami_storage::conformance::run_conformance_suite(&provider).await;
}
```

If it passes, the provider is interchangeable with every other conforming backend.
