# Filesystem (`loami-storage-fs`)

A local-filesystem `StorageProvider` (`file://`), rooted at a directory and built on `object_store`'s
`LocalFileSystem`. For local development and single-node persistence.

## Storage mechanism

Objects are stored as files beneath the root directory; reads, range reads, writes, listing, and
metadata all go through `object_store`. ETags are derived from file metadata.

## Callouts

- **Conditional `Update` (compare-and-swap) is emulated.** `object_store`'s local backend implements
  atomic `Create` but not conditional `Update`, so this provider serializes **all** writes through an
  in-process lock, reads the current ETag, and overwrites only if it matches. This is correct under a
  **single-writer** model — one process owns the directory — and is *not* safe against a second OS
  process writing the same directory concurrently. Cross-process CAS via OS file locks is a planned
  enhancement.
- **The root directory must already exist.** The provider does not create it.
- Because all writes are serialized, write throughput is single-threaded per provider — fine for the
  single-node use it targets.

## Configuration & usage

```rust
use loami_storage_fs::FsProvider;

// The directory must already exist.
let store = FsProvider::new("./data")?;
```

After construction it is used like any provider — see [Using a provider](../storage.md#using-a-provider).
