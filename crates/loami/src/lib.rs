//! # Loami
//!
//! _Fertile ground for your backend._
//!
//! Loami is an embeddable backend substrate for early-stage apps — a document store, durable
//! queue, realtime websocket backplane, and background-job runner over one self-clustering,
//! churn-tolerant kernel whose only dependencies are **compute + blob storage**.
//!
//! ## Status
//!
//! ⚠️ **Pre-alpha / design phase.** The engine is not yet implemented. This crate is a
//! placeholder that exists so continuous integration, the test suite, documentation, and the
//! release pipeline are all in place from the very first commit. The public API below is a
//! stand-in and will change. See the [roadmap] for what is coming.
//!
//! [roadmap]: https://github.com/waviisoft/loami#roadmap

/// Returns the version of the `loami` crate, as reported by Cargo at build time.
///
/// This is a placeholder API used to exercise the build, test, and documentation pipelines
/// while the engine itself is being designed.
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

    #[test]
    fn version_matches_cargo_env() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
