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

Empty on creation, and optionally capped at a maximum number of stored value bytes — useful to bound
an in-memory store used by one part of an application while another stays unbounded. A write that
would push the total over the cap fails with `StorageError::QuotaExceeded`.

```rust
use loami_storage_memory::MemoryProvider;

let store = MemoryProvider::new();                      // unbounded
let bounded = MemoryProvider::with_max_bytes(64 << 20); // capped at 64 MiB
```

Via a connection string the cap rides in the URL as a query option, so it is configuration rather
than code — `mem://` is unbounded, `mem://?max_bytes=67108864` caps the store at 64 MiB. After
construction it is used like any provider — see [Using a provider](../storage.md#using-a-provider).
