# Releasing

Releases are **tag-driven**. Pushing a `vX.Y.Z` tag runs the
[`release`](https://github.com/waviisoft/loami/blob/main/.github/workflows/release.yml) workflow,
which verifies, checks the tag matches the crate version, and cuts a GitHub Release — with notes
auto-generated from the merged PRs and commits since the previous tag. The docs site is already
current: the version-bump commit that precedes the tag is merged to `main`, and that push redeploys
GitHub Pages on its own.

## Steps

1. **Bump the version** in `crates/loami/Cargo.toml` to `X.Y.Z`. Required — the release fails unless
   the tag matches the crate version.
2. Commit on `main` (via PR): `chore(release): vX.Y.Z`.
3. **Tag and push:**
   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

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

So while pre-`1.0` most releases are `0.0.Z` and `0.Y.0`; true major (`X.0.0`) bumps don't start
mattering until after `1.0`.
