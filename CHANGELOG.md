# Changelog

All notable changes to md-wiki are documented in this file.

## Unreleased

- Prepare the repository for public OSS distribution with MIT licensing, public Cargo metadata, packaging exclusions, contributor docs, security policy, issue templates, and release notes.
- Strengthen README onboarding for installation, minimal usage, safety, limitations, and package verification.
- Align requirements docs with the current large Markdown large path behavior.

## 0.1.0 - 2026-05-08

Initial public release preparation.

- Generate an offline Markdown wiki from local `.md` files.
- Build entry pages, h2/h3 fragments, hierarchical `_index.md` pages, tags, headings, links, backlinks, and unresolved link reports.
- Support Obsidian-style `[[wikilink]]`, aliases, related notes, `wiki: false`, and `fragment: false`.
- Generate agentic search metadata, agent guide, page catalog, and term index.
- Keep generation local, deterministic, non-destructive, and independent of AI or external APIs.
