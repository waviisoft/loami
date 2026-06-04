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
| `list` | enumerate objects under a key prefix |

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

More providers (local filesystem, Azure Blob, …) are tracked on the [roadmap](./roadmap.md).

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
the CAS semantics above. Enable the `conformance` feature of `loami-storage` in your dev-dependencies
and call it from a test:

```rust
#[tokio::test]
async fn passes_conformance() {
    let provider = MyProvider::new();
    loami_storage::conformance::run_conformance_suite(&provider).await;
}
```

If it passes, the provider is interchangeable with every other conforming backend.
