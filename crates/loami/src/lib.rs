//! # Loami
//!
//! _Fertile ground for your backend._
//!
//! Loami is an embeddable backend substrate for early-stage apps — a document store, durable
//! queue, realtime websocket backplane, and background-job runner over one self-clustering,
//! churn-tolerant kernel whose only dependencies are **compute + blob storage**.

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

    #[test]
    fn version_matches_cargo_env() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
