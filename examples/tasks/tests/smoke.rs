//! Smoke tests: the example runs with zero services (`mem://`) and persists on `file://`. These run
//! in the workspace test suite on every build, continuously proving dev/prod parity.

use loami::Loami;
use loami_example_tasks::{connect, run};
use serde_json::json;

#[tokio::test]
async fn runs_on_memory() {
    // The zero-setup path: the full walkthrough on the built-in in-memory backend.
    let db = Loami::connect("mem://").expect("connect mem");
    run(&db).await.expect("run on mem");
}

#[tokio::test]
async fn runs_on_filesystem() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("file://{}", dir.path().display());
    let db = connect(&url).expect("connect file");
    run(&db).await.expect("run on file");
}

#[tokio::test]
async fn data_persists_across_connections_on_filesystem() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("file://{}", dir.path().display());

    // Write a document through one connection...
    let id = {
        let db = connect(&url).expect("connect file");
        db.collection("notes")
            .unwrap()
            .insert(json!({ "body": "remember this" }))
            .await
            .unwrap()
    };

    // ...and read it back through a fresh one rooted at the same directory — the local-dev story.
    let db = connect(&url).expect("reconnect file");
    let note = db.collection("notes").unwrap().get(&id).await.unwrap();
    assert_eq!(note.unwrap()["body"], json!("remember this"));
}
