//! A reusable conformance suite that exercises a [`StorageProvider`] against the contract.
//!
//! A provider that passes [`run_conformance_suite`] behaves interchangeably with every other
//! conforming provider — including the conditional-write (compare-and-swap) semantics that Loami
//! relies on. Enable the `conformance` feature (typically as a dev-dependency) and call it from a
//! test:
//!
//! ```ignore
//! #[tokio::test]
//! async fn passes_conformance() {
//!     let provider = MyProvider::new();
//!     loami_storage::conformance::run_conformance_suite(&provider).await;
//! }
//! ```
//!
//! Each check uses keys under the `conformance/` prefix and cleans up after itself, so it is safe to
//! run against a persistent backend.

use bytes::Bytes;

use crate::{Etag, ObjectKey, PutOptions, StorageError, StorageProvider};

/// Runs the full conformance suite against `provider`, panicking on the first violation.
pub async fn run_conformance_suite(provider: &dyn StorageProvider) {
    put_get_roundtrip(provider).await;
    get_missing_is_not_found(provider).await;
    head_reports_size_and_etag(provider).await;
    range_reads(provider).await;
    list_by_prefix(provider).await;
    delete_is_idempotent(provider).await;
    create_mode_cas(provider).await;
    update_mode_cas(provider).await;
}

fn key(name: &str) -> ObjectKey {
    ObjectKey::new(format!("conformance/{name}"))
}

async fn put_get_roundtrip(provider: &dyn StorageProvider) {
    let key = key("roundtrip");
    provider.delete(&key).await.expect("delete should not fail");

    let data = Bytes::from_static(b"hello world");
    let result = provider
        .put(&key, data.clone(), PutOptions::overwrite())
        .await
        .expect("put should succeed");
    assert!(
        !result.etag.as_str().is_empty(),
        "put must return a non-empty etag"
    );

    let got = provider.get(&key).await.expect("get should succeed");
    assert_eq!(got.data, data, "get must return exactly what was put");
    assert_eq!(
        got.meta.etag, result.etag,
        "get must return the etag of the bytes it returned, so callers never need a separate head"
    );

    provider.delete(&key).await.expect("cleanup delete");
}

async fn get_missing_is_not_found(provider: &dyn StorageProvider) {
    let key = key("missing");
    provider.delete(&key).await.expect("delete should not fail");

    match provider.get(&key).await {
        Err(StorageError::NotFound { .. }) => {}
        other => panic!("expected NotFound for a missing object, got {other:?}"),
    }
}

async fn head_reports_size_and_etag(provider: &dyn StorageProvider) {
    let key = key("head");
    let data = Bytes::from_static(b"0123456789"); // 10 bytes
    let put = provider
        .put(&key, data, PutOptions::overwrite())
        .await
        .expect("put should succeed");

    let meta = provider.head(&key).await.expect("head should succeed");
    assert_eq!(meta.size, 10, "head must report the correct size");
    assert_eq!(meta.etag, put.etag, "head etag must match the put etag");

    // Overwriting must change the ETag.
    let put2 = provider
        .put(
            &key,
            Bytes::from_static(b"changed"),
            PutOptions::overwrite(),
        )
        .await
        .expect("overwrite should succeed");
    assert_ne!(
        put2.etag, put.etag,
        "overwriting an object must produce a new etag"
    );

    provider.delete(&key).await.expect("cleanup delete");
}

async fn range_reads(provider: &dyn StorageProvider) {
    let key = key("range");
    provider
        .put(
            &key,
            Bytes::from_static(b"0123456789"),
            PutOptions::overwrite(),
        )
        .await
        .expect("put should succeed");

    let middle = provider
        .get_range(&key, 2..5)
        .await
        .expect("range read should succeed");
    assert_eq!(
        middle.data,
        Bytes::from_static(b"234"),
        "range read must return the requested slice"
    );

    match provider.get_range(&key, 5..100).await {
        Err(StorageError::InvalidRange { .. }) => {}
        other => panic!("expected InvalidRange for an out-of-bounds read, got {other:?}"),
    }

    provider.delete(&key).await.expect("cleanup delete");
}

async fn list_by_prefix(provider: &dyn StorageProvider) {
    let prefix = "conformance/list/";
    // Start from a clean slate under the prefix.
    for meta in provider.list(prefix).await.expect("list should succeed") {
        provider.delete(&meta.key).await.expect("cleanup delete");
    }

    let a = ObjectKey::new(format!("{prefix}a"));
    let b = ObjectKey::new(format!("{prefix}b"));
    provider
        .put(&a, Bytes::from_static(b"a"), PutOptions::overwrite())
        .await
        .expect("put a");
    provider
        .put(&b, Bytes::from_static(b"b"), PutOptions::overwrite())
        .await
        .expect("put b");

    let mut listed: Vec<String> = provider
        .list(prefix)
        .await
        .expect("list should succeed")
        .into_iter()
        .map(|m| m.key.as_str().to_owned())
        .collect();
    listed.sort();
    assert_eq!(
        listed,
        vec![a.as_str().to_owned(), b.as_str().to_owned()],
        "list must return exactly the keys under the prefix"
    );

    provider.delete(&a).await.expect("cleanup a");
    provider.delete(&b).await.expect("cleanup b");
}

async fn delete_is_idempotent(provider: &dyn StorageProvider) {
    let key = key("delete");
    provider
        .put(&key, Bytes::from_static(b"x"), PutOptions::overwrite())
        .await
        .expect("put should succeed");

    provider.delete(&key).await.expect("first delete");
    match provider.get(&key).await {
        Err(StorageError::NotFound { .. }) => {}
        other => panic!("object should be gone after delete, got {other:?}"),
    }
    // Deleting again must still succeed.
    provider
        .delete(&key)
        .await
        .expect("second delete is idempotent");
}

async fn create_mode_cas(provider: &dyn StorageProvider) {
    let key = key("create");
    provider.delete(&key).await.expect("delete should not fail");

    let original = Bytes::from_static(b"original");
    provider
        .put(&key, original.clone(), PutOptions::create())
        .await
        .expect("create on an absent key should succeed");

    match provider
        .put(&key, Bytes::from_static(b"other"), PutOptions::create())
        .await
    {
        Err(StorageError::AlreadyExists { .. }) => {}
        other => panic!("create on an existing key must fail with AlreadyExists, got {other:?}"),
    }

    let got = provider.get(&key).await.expect("get should succeed");
    assert_eq!(
        got.data, original,
        "a failed create must not modify the object"
    );

    provider.delete(&key).await.expect("cleanup delete");
}

async fn update_mode_cas(provider: &dyn StorageProvider) {
    let key = key("update");
    provider.delete(&key).await.expect("delete should not fail");

    // Update against a non-existent object must fail (Precondition or NotFound, per backend).
    match provider
        .put(
            &key,
            Bytes::from_static(b"v1"),
            PutOptions::update(Etag::new("nope")),
        )
        .await
    {
        Err(StorageError::Precondition { .. } | StorageError::NotFound { .. }) => {}
        other => panic!("update on a missing object must fail, got {other:?}"),
    }

    let v1 = provider
        .put(&key, Bytes::from_static(b"v1"), PutOptions::overwrite())
        .await
        .expect("seed v1");

    // Update with the correct ETag succeeds and yields a fresh ETag.
    let v2 = provider
        .put(
            &key,
            Bytes::from_static(b"v2"),
            PutOptions::update(v1.etag.clone()),
        )
        .await
        .expect("update with the current etag should succeed");
    assert_ne!(
        v2.etag, v1.etag,
        "a successful update must produce a new etag"
    );
    assert_eq!(
        provider.get(&key).await.expect("get").data,
        Bytes::from_static(b"v2"),
        "update must have written the new value"
    );

    // Update with the now-stale ETag fails and leaves the object untouched.
    match provider
        .put(&key, Bytes::from_static(b"v3"), PutOptions::update(v1.etag))
        .await
    {
        Err(StorageError::Precondition { .. }) => {}
        other => panic!("update with a stale etag must fail with Precondition, got {other:?}"),
    }
    assert_eq!(
        provider.get(&key).await.expect("get").data,
        Bytes::from_static(b"v2"),
        "a failed update must not modify the object"
    );

    provider.delete(&key).await.expect("cleanup delete");
}
