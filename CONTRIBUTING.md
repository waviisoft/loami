# Contributing to Loami

Loami is in its **pre-alpha / design phase**. Contributions, ideas, and design feedback are welcome
— but expect the architecture to move quickly. See the [roadmap](./docs/src/roadmap.md) for current
direction.

## Ground rules

- **Every change ships with tests.** New behavior needs unit and/or integration tests; bug fixes
  need a test that fails before the fix and passes after. CI does not accept untested behavior.
- **Keep CI green.** `fmt`, `clippy -D warnings`, and the full test suite must pass.
- **Document public items.** `missing_docs` is a warning at the workspace level; public API needs
  doc comments (with an example where it helps).

## Local development

```sh
# Format, lint, test — the same gates CI runs.
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all --all-features

# Build the docs site locally (requires mdBook: `cargo install mdbook`).
mdbook serve docs --open      # the guide
cargo doc --no-deps --open    # the API reference
```

## Pull requests

1. Branch from `main` (e.g. `feat/...`, `fix/...`, `docs/...`, `chore/...`).
2. Make the change **with tests**.
3. Open a PR — CI runs fmt, clippy, tests (Linux + macOS), and a docs build check. Release notes are
   generated from PR titles/commits, so write a clear, descriptive PR title.

## Releases

See [docs/src/releasing.md](./docs/src/releasing.md).

## License

By contributing, you agree that your contributions are licensed under the [MIT License](./LICENSE).
