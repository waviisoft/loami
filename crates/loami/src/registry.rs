//! Connection-string scheme registry.
//!
//! [`Loami::connect`](crate::Loami::connect) resolves a URL's scheme through a [`Registry`]. The
//! [`Default`] registry holds only the in-memory provider; register others to support more schemes
//! and pass the registry to [`Loami::connect_with`](crate::Loami::connect_with).

use std::collections::HashMap;
use std::sync::Arc;

use loami_storage::StorageProvider;

use crate::{Error, Result};

/// Builds a provider from the part of a connection string after `scheme://`.
type Factory = Arc<dyn Fn(&str) -> Result<Arc<dyn StorageProvider>> + Send + Sync>;

/// Maps connection-string schemes (e.g. `mem`) to provider constructors.
///
/// Nothing about the available backends is hard-coded into `connect`; a backend is available exactly
/// when its scheme is registered here. [`Registry::default`] registers only the in-memory provider
/// (`mem://`); register additional providers — a filesystem or cloud backend, or your own — before
/// connecting.
#[derive(Clone)]
pub struct Registry {
    factories: HashMap<String, Factory>,
}

impl Registry {
    /// An empty registry with no schemes registered.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Registers `factory` for `scheme` (the part before `://`), replacing any prior registration.
    /// The factory receives the part of the URL after `scheme://`.
    pub fn register(
        &mut self,
        scheme: impl Into<String>,
        factory: impl Fn(&str) -> Result<Arc<dyn StorageProvider>> + Send + Sync + 'static,
    ) -> &mut Self {
        self.factories.insert(scheme.into(), Arc::new(factory));
        self
    }

    /// The registered schemes, sorted — useful for diagnostics.
    #[must_use]
    pub fn schemes(&self) -> Vec<&str> {
        let mut schemes: Vec<&str> = self.factories.keys().map(String::as_str).collect();
        schemes.sort_unstable();
        schemes
    }

    /// Resolves `url` to a provider via its scheme.
    pub(crate) fn resolve(&self, url: &str) -> Result<Arc<dyn StorageProvider>> {
        let (scheme, rest) = url.split_once("://").ok_or_else(|| Error::Url {
            url: url.to_owned(),
            reason: "expected a connection string like \"scheme://...\"",
        })?;
        match self.factories.get(scheme) {
            Some(factory) => factory(rest),
            None => Err(Error::UnknownScheme {
                url: url.to_owned(),
                scheme: scheme.to_owned(),
                registered: self.schemes().join(", "),
            }),
        }
    }
}

impl Default for Registry {
    /// A registry with only the in-memory provider (`mem://`) registered — the engine's single
    /// built-in default. Register any other provider (a filesystem or cloud backend, or your own)
    /// before connecting.
    fn default() -> Self {
        let mut registry = Self::empty();
        registry.register("mem", |_rest| {
            let provider: Arc<dyn StorageProvider> =
                Arc::new(loami_storage_memory::MemoryProvider::new());
            Ok(provider)
        });
        registry
    }
}
