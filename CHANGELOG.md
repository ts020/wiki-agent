# Changelog

All notable changes to md-wiki are documented in this file.

## 0.1.1 - 2026-05-11

Distribution hardening release. No behavior change in the `md-wiki` binary itself.

- Build and publish `aarch64-unknown-linux-gnu` tarballs in addition to the four targets shipped with 0.1.0.
- Tag-trigger release workflow on both `v<semver>` and bare `<semver>` tag shapes and resolve release notes dynamically from `docs/releases/`.
- Pin GitHub Actions to commit SHAs in `release.yml` and `verify.yml` so an upstream tag rewrite cannot change what runs.
- `install.sh` now verifies the sha256 of the downloaded tarball by extracting the expected hash with `awk`, computing the actual hash, and string-comparing — closing a fail-open path where a malformed `checksums.txt` would have been accepted by `sha256sum -c`.
- `install.sh` fails closed when `checksums.txt` is missing, lists no entry for the asset, or no sha256 tool is available. Override with `MD_WIKI_SKIP_CHECKSUM=1` at your own risk.
- `install.sh` extracts release archives into a dedicated subdirectory and rejects extracted binary paths that escape it.
- Document the release flow in `docs/RELEASING.md` and expand `.gitignore` for editor/OS noise.

## 0.1.0 - 2026-05-08

Initial public release.

- Generate an offline Markdown wiki from local `.md` files.
- Build entry pages, h2/h3 fragments, hierarchical `_index.md` pages, tags, headings, links, backlinks, and unresolved link reports.
- Support Obsidian-style `[[wikilink]]`, aliases, related notes, `wiki: false`, and `fragment: false`.
- Generate agentic search metadata, agent guide, page catalog, and term index.
- Provide GitHub Release archives that can be installed with `curl | sh`.
- Keep generation local, deterministic, non-destructive, and independent of AI or external APIs.
- Prepare the repository for public OSS distribution with MIT licensing, public Cargo metadata, packaging exclusions, contributor docs, security policy, issue templates, and release notes.
- Strengthen README onboarding for installation, minimal usage, safety, limitations, and package verification.
- Align requirements docs with the current large Markdown large path behavior.
