# Releasing

Releases are **tag-driven**. Pushing a `vX.Y.Z` tag runs the
[`release`](https://github.com/waviisoft/loami/blob/main/.github/workflows/release.yml) workflow,
which verifies, cuts a GitHub Release, and refreshes the docs site.

> **Package publishing is deferred.** Loami does not yet publish to crates.io, npm, or PyPI — there
> is nothing concrete to ship while it is pre-alpha. Those steps will be added to the release
> workflow once the engine exists. See [Deferred](#deferred) below.

## Steps

1. **Update `CHANGELOG.md`** — move items out of `Unreleased` into a new `## [X.Y.Z]` section.
2. _(Hygiene)_ bump the version in `crates/loami/Cargo.toml` so the crate version tracks the tag.
3. Commit on `main` (via PR): `chore(release): vX.Y.Z`.
4. **Tag and push:**
   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

## What the workflow does

1. **verify** — `fmt`, `clippy -D warnings`, and the full test suite.
2. **github-release** — creates a GitHub Release with auto-generated notes.
3. **refresh-docs** — triggers the `docs` workflow so GitHub Pages reflects the release.

## Rehearsal

Run the `release` workflow manually (`workflow_dispatch`) to execute just the verify gate without
creating a release.

## Versioning

[Semantic Versioning](https://semver.org/). While on `0.x`, breaking changes may land in minor
releases — expected during pre-alpha.

## Deferred

When there is something concrete to publish, add a publish job to the release workflow:

| Target | How | Secret |
| --- | --- | --- |
| crates.io | `cargo publish -p loami` | `CARGO_REGISTRY_TOKEN` |
| npm (binding) | `npm publish` from `crates/loami-node` | `NPM_TOKEN` |
| PyPI (binding) | `maturin publish` from `crates/loami-py` | PyPI token |
