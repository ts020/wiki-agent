# md-wiki Agent Context

## Project Goal

`md-wiki` is a Rust CLI that turns local Markdown notes into an offline personal wiki tree.

The tool indexes Markdown structure rather than summarizing prose. It preserves source `.md` files, converts supported wiki links in generated output, and builds navigation through tags, headings, links, backlinks, fragments, and unresolved-link reports.

## Source Of Truth

- Requirements: `docs/要件定義/`
- Requirements index: `docs/要件定義/index.md`
- v1 acceptance criteria: `docs/要件定義/13-受け入れ基準.md`
- Future-scope items: `docs/要件定義/14-将来拡張.md`

If implementation details conflict, treat `docs/要件定義/` as authoritative.

## Scope Rules

- Target Markdown files only. Do not add source-code analysis or symbol extraction.
- Do not use AI, LLMs, external APIs, or network services for wiki generation.
- Keep generation non-destructive: never modify input Markdown files.
- `init` performs initial generation; `add` recomputes whole-wiki consistency and applies only filesystem diffs based on `.md-wiki/manifest.json`.
- Do not implement future-scope items unless the user explicitly asks for them.

## Architecture

- CLI entrypoint exposes `init` and `add` subcommands.
- Scanner collects eligible Markdown files.
- Note ingestion parses frontmatter, headings, wiki links, tags, aliases, related notes, and fragment settings.
- Fragment logic splits notes into entry pages, h2 fragments, and h3 child fragments when required.
- Relation and render modules produce indexes, backlinks, related sections, unresolved links, and output pages.

Default output directory: `./md-wiki`.

## Commands

```sh
cargo build
cargo run -- init <INPUT> [-r|--recursive] [-o|--out <DIR>]
cargo run -- add [PATH] [-o|--out <DIR>]
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Run `cargo test` after behavior changes. For formatting-only changes, use the relevant formatting check.

## Commit Rule

Commit messages should explain why the change is needed, not just list what changed. Do not include AI signatures such as `Co-Authored-By`.
