# Changelog

All notable changes to Loami are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project scaffold: Cargo workspace with a placeholder `loami` crate (`version()`), unit
  tests, and an integration smoke test.
- Continuous integration: `rustfmt`, `clippy -D warnings`, and tests on Linux and macOS.
- Documentation site on GitHub Pages: an mdBook guide plus the rustdoc API reference, redeployed on
  every push to `main`.
- Tag-driven release pipeline: verify → publish to crates.io → GitHub Release → docs refresh, with a
  `workflow_dispatch` dry-run for rehearsal.
- Dependabot for Cargo and GitHub Actions.

[Unreleased]: https://github.com/waviisoft/loami/commits/main
