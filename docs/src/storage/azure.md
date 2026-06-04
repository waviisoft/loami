# Azure Blob (`loami-storage-azure`)

An Azure Blob Storage `StorageProvider` (`azure://`), built on `object_store`'s `MicrosoftAzure`.

## Storage mechanism

Objects are blobs in an Azure Storage container; reads, range reads, writes, listing, and metadata
all go through `object_store`. Azure Blob supports conditional `Update` (compare-and-swap by ETag)
and lazy listing **natively**, so this provider maps straight onto the contract — no emulation and no
write lock, unlike the filesystem provider.

## Callouts

- **Native CAS and lazy listing** — conditional `Update` is the real thing, not emulated.
- **The container must already exist.** The provider does not create it.
- **Local/CI testing uses [Azurite](https://github.com/Azure/Azurite)**, the official emulator.

## Authentication

Credentials use the standard Azure conventions, delegated entirely to `object_store` — nothing
Loami-specific. The simple path reads the usual `AZURE_STORAGE_*` environment variables (account key,
SAS, service principal, managed identity), the same ones the Azure SDK and CLI use, so a client
authenticates the way it already does.

## Configuration & usage

```rust
use loami_storage_azure::{AzureProvider, MicrosoftAzureBuilder};

// Simple: credentials from the standard Azure environment; just name the container.
let store = AzureProvider::from_env("my-container")?;

// Full control: hand in a configured object_store builder (re-exported) for any auth or endpoint
// object_store supports — including the Azurite emulator.
let store = AzureProvider::from_builder(
    MicrosoftAzureBuilder::new()
        .with_account("account")
        .with_access_key("key")
        .with_container_name("my-container"),
)?;
```

After construction it is used like any provider — see [Using a provider](../storage.md#using-a-provider).
