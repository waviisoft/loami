# Releasing

Releases are **tag-driven**. Pushing a `vX.Y.Z` tag runs the
[`release`](https://github.com/waviisoft/loami/blob/main/.github/workflows/release.yml) workflow,
which verifies, cuts a GitHub Release, and refreshes the docs site.

> **Package publishing is deferred.** Loami does not yet publish to crates.io, npm, or PyPI — there
> is nothing concrete to ship while it is pre-alpha. Those steps will be added to the release
> workflow once the engine exists. See [Deferred](#deferred) below.

## Steps

1. **Bump the version** in `crates/loami/Cargo.toml` to `X.Y.Z`. This is **required** — the release
   fails if the tag does not match the crate version (see below).
2. **Update `CHANGELOG.md`** — move items out of `Unreleased` into a new `## [X.Y.Z]` section.
3. Commit on `main` (via PR): `chore(release): vX.Y.Z`.
4. **Tag and push:**
   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

## What the workflow does

1. **verify** — `fmt`, `clippy -D warnings`, and the full test suite.
2. **tag-version-match** — fails the release unless the `vX.Y.Z` tag equals the
   `crates/loami/Cargo.toml` version, so the Cargo version always tracks releases.
3. **github-release** — creates a GitHub Release with auto-generated notes.
4. **refresh-docs** — triggers the `docs` workflow so GitHub Pages reflects the release.

## Rehearsal

Run the `release` workflow manually (`workflow_dispatch`) to execute just the verify gate without
creating a release.

## Choosing the version

Versions are [Semantic Versioning](https://semver.org/) `MAJOR.MINOR.PATCH`. The "size" of a release
is just which field you bump — and bumping a field resets the lower fields to `0`. The
[steps](#steps) above are identical for all three; only the number you pick differs.

| Release | Bump | Example | When |
| --- | --- | --- | --- |
| **Patch** `x.y.Z` | last field | `0.3.2 → 0.3.3` | backwards-compatible **bug fixes** only |
| **Minor** `x.Y.0` | middle, reset patch | `0.3.3 → 0.4.0` | backwards-compatible **new features** |
| **Major** `X.0.0` | first, reset both | `0.4.0 → 1.0.0` | **breaking** (incompatible API) changes |

### Pre-1.0 nuance (we are here)

While `MAJOR` is `0`, SemVer treats the API as unstable and the convention shifts left by one (this
is also how Cargo resolves `0.x` dependencies):

- **`0.Y.0`** (bump minor) is the effective **"breaking change"** release while pre-1.0.
- **`0.y.Z`** (bump patch) covers **both features and fixes**.
- Bump to **`1.0.0`** only when committing to API stability.

So during pre-alpha most releases are `0.0.Z` and `0.Y.0`; true major (`X.0.0`) bumps don't start
mattering until after `1.0`.

## Deferred

When there is something concrete to publish, add a publish job to the release workflow:

| Target | How | Secret |
| --- | --- | --- |
| crates.io | `cargo publish -p loami` | `CARGO_REGISTRY_TOKEN` |
| npm (binding) | `npm publish` from `crates/loami-node` | `NPM_TOKEN` |
| PyPI (binding) | `maturin publish` from `crates/loami-py` | PyPI token |
