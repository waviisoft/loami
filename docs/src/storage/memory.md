# Memory (`loami-storage-memory`)

An in-process `StorageProvider` backed by a `HashMap`, with monotonically increasing ETags. Used for
tests, CI, ephemeral workloads, and the `mem://` scheme.

## Storage mechanism

Objects live in process memory behind a mutex; each write mints a new ETag from a monotonic counter.
It is the contract's **reference implementation** — hand-written rather than wrapping a third-party
object store, so the [conformance suite](../storage.md#implementing-a-provider) is validated against
an independent backend.

## Callouts

- **Not durable.** All data is lost when the process exits or the provider is dropped.
- **Listing is snapshot-then-stream.** `list` collects matching entries under the lock and streams
  that snapshot, so it is not constant-memory like the streaming backends — fine for its role.
- **Order is incidental.** It happens to return keys sorted (for test determinism), but the contract
  leaves list order unspecified — do not rely on it.

## Configuration & usage

No configuration — it is empty on creation:

```rust
use loami_storage_memory::MemoryProvider;

let store = MemoryProvider::new();
```

After construction it is used like any provider — see [Using a provider](../storage.md#using-a-provider).
