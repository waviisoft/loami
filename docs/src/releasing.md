# Releasing

Releases are **tag-driven**. Pushing a `vX.Y.Z` tag runs the
[`release`](https://github.com/waviisoft/loami/blob/main/.github/workflows/release.yml) workflow,
which verifies, publishes, cuts a GitHub Release, and refreshes the docs site.

## Steps

1. **Update the version** in `crates/loami/Cargo.toml`.
2. **Update `CHANGELOG.md`** — move items out of `Unreleased` into a new `## [X.Y.Z]` section.
3. Commit on `main` (via PR): `chore(release): vX.Y.Z`.
4. **Tag and push:**
   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

## What the workflow does

1. **verify** — `fmt`, `clippy -D warnings`, and the full test suite.
2. **publish-crate** — checks the tag matches the crate version, then `cargo publish` to crates.io.
3. **github-release** — creates a GitHub Release with auto-generated notes.
4. **refresh-docs** — triggers the `docs` workflow so GitHub Pages reflects the release.

## Rehearsal

Run the `release` workflow manually (`workflow_dispatch`) with **dry_run = true** to exercise the
verify + `cargo publish --dry-run` path without releasing anything.

## Required secrets

| Secret | Used for |
| --- | --- |
| `CARGO_REGISTRY_TOKEN` | `cargo publish` to crates.io |
| `NPM_TOKEN` _(future)_ | publishing the npm binding once it exists |

## Versioning

[Semantic Versioning](https://semver.org/). While on `0.x`, breaking changes may land in minor
releases — expected during pre-alpha.
