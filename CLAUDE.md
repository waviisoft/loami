# Working in this repo

Loami is an embeddable backend kernel written in Rust — a document store,
durable queue, realtime backplane, and job runner over one self-clustering
kernel whose only dependencies are compute + blob storage. It is in its
**pre-alpha / design phase**, so expect the architecture to move quickly. This
file records how an agent (or contributor) should **work** in this repo. The
contribution rules for humans live in [CONTRIBUTING.md](CONTRIBUTING.md).

## Agents available on this repo

Reusable subagents live in `.claude/agents/` and are shared by everyone who
opens this repo. Prefer them over reinventing the workflow:

- **`code-reviewer`** — review a diff, branch, or PR across correctness,
  security, privacy, observability, coding principles, tests, and docs.
- **`pr-author`** — take a change from a branch to a squash-merged PR, using
  `code-reviewer` for the independent review.
- **`tdd-engineer`** — implement a feature or fix a bug test-first (Red → Green
  → Refactor), wiring CI to run the tests.
- **`project-setup`** — shape or audit repo structure so it reflects the project
  as it is now, with every feature documented under `docs/`.

`.claude/commands/changelog.md` and the `pr-summary` skill (`.claude/skills/`)
support the same workflow.

## Working agreements

- Match the style of surrounding code; don't introduce new dependencies without
  reason.
- Keep changes scoped to the request; flag unrelated issues instead of fixing
  them silently.
- Verify before declaring a change done: run the relevant tests (or exercise the
  affected flow) and report honestly — what you ran, what passed or failed, and
  what you skipped. Don't claim done on an unverified change.

### Handle uncertainty by asking, not guessing

- When the request is ambiguous, or a decision is genuinely the user's to make,
  ask before acting rather than picking a direction and running with it.
- **A declined or timed-out picker is not permission to pick a default.** When
  you ask the user to choose and they decline, dismiss, or let it time out,
  treat the needed detail as still missing — stop and get the answer another
  way, or surface that you're blocked, rather than guessing.

### Safety

- **No secrets.** Never commit API keys, tokens, credentials, internal
  hostnames, or private paths. Read your own diff before pushing — the
  `.gitignore` is a backstop, not a substitute.

## Issues

- **Issues are dual-purpose.** An issue is both a **plan** — the plan of record
  for a unit of work, before and during implementation — and a **bug/defect
  record** documenting a problem to be fixed. The
  [issue templates](.github/ISSUE_TEMPLATE) encode both.
- **The description is the living plan.** The issue **description (body)** is the
  single, current source of truth. It is a living document: as understanding
  evolves, edit the description so it always reflects the present intent and
  state. A reader should understand the present intent from the description
  alone, without reconstructing it from the comment thread.
- **Comments capture what changed and why.** Whenever the description is edited,
  add a comment explaining what changed and the reasoning behind it. Comments
  are the audit trail; the description is the present state.
- **Future work goes in the tracker, not the code.** No `TODO`/`FIXME` markers
  and no "coming soon" stubs — file an issue instead.

## Pull requests

- **Every change ships via a pull request. Never commit directly to `main`.**
  Branch first, using a short kebab-case name describing the change
  (`feat/...`, `fix/...`, `docs/...`, `chore/...`).
- **Every change ships with tests.** New behavior needs unit and/or integration
  tests; a bug fix needs a test that fails before the fix and passes after. CI
  does not accept untested behavior.
- **PRs are squashed and merged.** The squash commit **subject** ends with the
  PR-number suffix (`Add widget caching (#123)`); the squash commit **body** is
  the prose **Summary** paragraphs of the PR description, verbatim — so keep the
  Summary self-contained.
- Run your **own independent code review** (hand the change to the
  `code-reviewer` agent) before requesting merge — don't rely solely on CI.
- When responding to review comments, you may decide a change is or isn't
  warranted, but **ask for permission or clarification before acting** — never
  silently apply or dismiss review feedback.
- Keep the PR description current as the branch evolves. See the
  [PR template](.github/pull_request_template.md) for the required format.

## Docs track code

A change to user-facing behavior updates the guide (`docs/`) in the **same** PR,
with a usage example — not just the rustdoc API reference. A PR that changes
behavior without touching the relevant doc is incomplete.

## Project facts

- **Build / test / lint** — the same gates CI runs:
  ```sh
  cargo fmt --all --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all --all-features
  ```
- **Docs site** (mdBook, requires `cargo install mdbook`):
  ```sh
  mdbook serve docs --open      # the guide, sources under docs/src/
  cargo doc --no-deps --open    # the API reference
  ```
- **Layout:** a Cargo workspace. The engine crate is `crates/loami`; storage
  backends are the `crates/loami-storage*` crates; runnable examples live under
  `examples/`. The guide lives in `docs/src/` (see `docs/src/SUMMARY.md`), brand
  assets in `brand/`.
- **Gotchas:**
  - `clippy` runs with `-D warnings` in CI — a warning fails the build.
  - `missing_docs` is a workspace-level warning: public items need doc comments.
  - `unsafe_code` is **forbidden** workspace-wide.
  - The toolchain is pinned to stable via `rust-toolchain.toml`.
  - Releases are cut by tagging `vX.Y.Z`; the tag must match the `crates/loami`
    Cargo version, and release notes are generated from merged PRs — so write a
    clear, descriptive PR title.
