# Loami tasks example

A tiny tasks CRUD store over schemaless JSON documents — the getting-started example for
[Loami](../../README.md). The same code runs against any backend; only the connection string changes.
See the [getting-started guide](../../docs/src/getting-started.md) for the full walkthrough.

## Running it

The backend is chosen by `LOAMI_URL` (default `mem://`); nothing else changes.

```sh
# in-memory (default) — nothing to set up
cargo run -p loami-example-tasks

# local filesystem — persists in ./loami-data, inspectable on disk
LOAMI_URL=file://./loami-data cargo run -p loami-example-tasks

# Azure Blob — needs the feature and the standard AZURE_STORAGE_* credentials
LOAMI_URL=azure://my-container cargo run -p loami-example-tasks --features azure
```

Run the `file://` command twice and the second run reports the tasks the first left behind — same
code, now durable.

## How it's tested

[`tests/smoke.rs`](./tests/smoke.rs) runs the full walkthrough on `mem://` and again on `file://`,
and one test writes through a `file://` connection and reads the data back through a fresh one to
confirm it persisted — no running services required.

These run in CI on **every pull request**: the `test` job in
[`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) executes `cargo test --all --all-features`
on Linux and macOS, and this example is a workspace member, so its smoke tests are part of that run.
That keeps the example continuously proving it works as Loami evolves.

You can run them yourself with:

```sh
cargo test -p loami-example-tasks
```
