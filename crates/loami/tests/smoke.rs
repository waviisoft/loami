//! Integration smoke test.
//!
//! Confirms the crate links and its public surface is reachable from an external test target.
//! Replaced/expanded with real behavior tests as the engine lands.

#[test]
fn version_is_reported_from_integration_test() {
    assert_eq!(loami::version(), env!("CARGO_PKG_VERSION"));
}
