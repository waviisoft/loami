//! # Loami
//!
//! _Fertile ground for your backend._
//!
//! Loami is an embeddable document store backed by pluggable storage providers behind the
//! [`StorageProvider`](loami_storage::StorageProvider) contract, so the same code runs in tests,
//! locally, and in production. Open a store with a connection string (or any provider) and work with
//! collections of schemaless JSON documents:
//!
//! ```
//! use loami::Loami;
//! use serde_json::json;
//!
//! # async fn run() -> loami::Result<()> {
//! // mem:// is the built-in default; register other providers to add their schemes.
//! let db = Loami::connect("mem://").await?;
//! let tasks = db.collection("tasks")?;
//! let id = tasks.insert(json!({ "title": "ship loami", "done": false })).await?;
//! tasks.update(&id, json!({ "title": "ship loami", "done": true })).await?;
//! assert_eq!(tasks.find(json!({ "done": true })).await?.len(), 1);
//! # Ok(())
//! # }
//! ```

mod document;
mod engine;
mod error;
mod registry;

pub use document::{DocId, Document};
pub use engine::{Collection, Loami};
pub use error::{Error, Result};
pub use registry::Registry;

/// Re-exported from `loami-storage`: the storage error wrapped by [`Error::Storage`]. Re-exported so
/// a consumer can match on it through `loami` alone, without a direct dependency on `loami-storage`.
pub use loami_storage::StorageError;

/// The built-in in-memory connection string — the engine's zero-setup default backend, registered by
/// [`Registry::default`]. Exposed so callers can reference the default without re-spelling the
/// literal `"mem://"`.
pub const MEM_URL: &str = "mem://";

/// Returns the version of the `loami` crate, as reported by Cargo at build time.
///
/// # Examples
///
/// ```
/// assert_eq!(loami::version(), env!("CARGO_PKG_VERSION"));
/// ```
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty(), "crate version should be reported");
    }
}
