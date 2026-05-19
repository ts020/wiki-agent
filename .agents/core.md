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
cargo run -- init [INPUT] [--no-recursive] [-o|--out <DIR>]
cargo run -- add [PATH] [-o|--out <DIR>]
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Run `cargo test` after behavior changes. For formatting-only changes, use the relevant formatting check.

## Decision Rewards

When implementing or reviewing, optimize first for general rules that cover broad behavior with small code. Correctness and verification remain mandatory; small code is high reward only when it preserves the requirements, determinism, readability, and tests.

### Generalization First

- Treat generalization as the top implementation reward: prefer a few rules, data shapes, and pipeline stages that cover many cases.
- Before adding a feature-specific branch, flag, type, renderer, parser path, or config knob, look for a way to express the behavior through the existing pipeline.
- A special case is acceptable only when the common rule cannot express the requirement clearly; make that reason explicit in the implementation notes or review.
- Prefer changing the model so the simple path handles more cases over adding another path beside it.

### Before Adding A Special Case

Ask these questions before coding a dedicated path:

1. Is this an extension of an existing rule, or a new feature-specific route?
2. Can the input be normalized earlier so the existing logic handles it?
3. Can common metadata, relation data, or render steps express this without a new mode?
4. Does this change let us delete an older branch, duplicate loop, temporary state, or narrow helper?
5. Are the tests proving the general rule, or only adding another example-shaped exception?

High-reward behavior:

- Keep changes aligned with `docs/要件定義/`, especially acceptance criteria and scope rules.
- One rule covers multiple input shapes, output kinds, or acceptance criteria.
- The feature fits existing scanner, ingestion, relation, or render flow without adding a parallel path.
- The change deletes more code than it adds while preserving or widening supported behavior.
- Tests check invariants and properties of the common rule, not just isolated examples.
- The implementation reduces branches, mutable state, duplicate loops, or narrow helper functions.
- Add or update focused tests for behavior changes, then run the relevant verification command.
- Protect user and generated work: do not revert unrelated changes or modify input Markdown files.
- Surface ambiguity, scope drift, and unverified assumptions before coding around them.
- In reviews, prioritize concrete bugs, regressions, missing tests, and requirement mismatches.

Low-reward behavior:

- Claiming completion without running or explaining verification.
- Implementing future-scope behavior without an explicit user request.
- Adding feature-specific branches, flags, config, types, parser paths, or render paths when a shared rule would cover the case.
- Creating a structure where the next related feature will likely need another dedicated path.
- Treating examples as separate implementations instead of discovering the rule that covers them.
- Increasing code size without increasing supported behavior, clarity, or verifiability.
- Broad refactors, abstractions, or formatting churn that are not needed for the task.
- Review comments based mainly on taste, style preference, or speculative risk.
- Code golf: making code shorter by making it harder to read, less deterministic, or harder to test.
- Hiding uncertainty, silently changing behavior, or relying on unstated inference.

## Commit Rule

Commit messages should explain why the change is needed, not just list what changed. Do not include AI signatures such as `Co-Authored-By`.
