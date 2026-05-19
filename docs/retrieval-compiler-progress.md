# md-wiki Retrieval Compiler Progress

Last updated: 2026-05-19
Workspace: `/Users/suga/conductor/workspaces/wiki-agent/kathmandu-v1`

## Current PR Scope

Retrieval compiler implementation included in the PR branch:

- `Cargo.toml` / `Cargo.lock`: add `serde_yaml`
- `src/lib.rs`: export `schema`
- `src/main.rs`: add `--schema` to `init` / `add`, add `context` subcommand
- `docs/retrieval-compiler-progress.md`: repository-visible progress tracker
- `src/schema.rs`: schema pack parsing, catalog generation, field catalogs, context pack rendering
- `tests/schema_context.rs`: acceptance coverage for schema/context vertical slice

Latest verified commands for the code changes:

- [x] `rtk proxy cargo fmt --check`
- [x] `rtk proxy cargo test --test schema_context` -> 8 passed
- [x] `rtk proxy cargo test` -> 166 passed
- [x] `rtk proxy cargo clippy -- -D warnings`
- [x] `rtk scripts/verify.sh`
  - includes `cargo fmt --check`
  - includes `cargo clippy -- -D warnings`
  - includes `cargo test`
  - includes `quality_score`, `large_md_gate`, and `agentic_search_gate`

Package check status:

- [x] `rtk cargo package --locked`
  - Packaged 83 files and verified `md-wiki-cli v0.1.3`.

## Acceptance Progress

Legend:

- `[x]` implemented and covered by automated tests
- `[~]` partially implemented; usable, but below full requirement quality
- `[ ]` not implemented or not mechanically verified

### Baseline Compile Layer

- [x] AC-01 CLI help
- [x] AC-02 single Markdown input
- [x] AC-03 recursive directory input by default
- [x] AC-04 `--no-recursive`
- [x] AC-05 scan exclusions
- [x] AC-06 wikilink resolution
- [x] AC-07 unresolved wikilinks
- [x] AC-08 tag index
- [x] AC-09 heading index
- [x] AC-10 link index
- [x] AC-11 backlinks
- [x] AC-12 reduced root `index.md`
- [x] AC-13 add/idempotency
- [x] AC-14 non-destructive input handling
- [x] AC-15 offline operation by design/local tests
- [x] AC-16 fragment navigation
- [x] AC-17 h3 re-split threshold
- [x] AC-18 `fragment: false`
- [x] AC-19 single h2 split
- [x] AC-20 no h2 entry-only note
- [x] AC-21 hierarchical `_index.md`
- [x] AC-22 root index sitemap only
- [x] AC-23 oversized page warning

### Retrieval Compiler Layer

- [x] AC-24 schema pack loading
  - Covered by `tests/schema_context.rs::schema_validation_rejects_undefined_context_fields`
  - Still worth adding explicit malformed YAML / missing required top-level field cases.

- [~] AC-25 catalog
  - Implemented: `.md-wiki/catalog.json` with schema id/version, generated path, source path/range, entities, tags, headings, extracted fields, outgoing links, backlinks.
  - Review fix: heading field evidence ranges now account for YAML frontmatter lines.
  - Gap: generated page source ranges are still coarse page/source ranges.

- [x] AC-26 field catalog
  - Covered by `tests/schema_context.rs::schema_init_generates_internal_catalog_and_field_catalogs`
  - Covered by `tests/schema_context.rs::add_with_schema_refreshes_catalog_and_field_catalogs`

- [~] AC-27 context pack generation
  - Implemented: `md-wiki context --wiki <DIR> --schema <YAML> --task <TASK>` outputs Markdown with `md_wiki_context`, recipe title, sections, and `Source Trail`.
  - Gap: candidate collection is currently simple field/entity/query matching.

- [x] AC-28 required field missing evidence
  - Covered by `tests/schema_context.rs::context_outputs_markdown_pack_with_missing_evidence_budget_and_determinism`

- [~] AC-29 context budget
  - Implemented: output is capped by `--budget` / recipe default.
  - Review fix: budget pruning preserves YAML frontmatter, context title, and `Source Trail` marker.
  - Gap: truncation is still coarse; FR-22 wants deterministic section/evidence pruning.

- [x] AC-30 retrieval determinism
  - Covered by `tests/schema_context.rs::context_outputs_markdown_pack_with_missing_evidence_budget_and_determinism`

- [~] AC-31 no inference / no network
  - Implemented by architecture: no LLM, embedding, external API, network, or schema code execution.
  - Gap: no dedicated offline/dependency gate yet.

## Next Tasks

1. Decide whether to persist schema usage across `add` in a future iteration.
   - Current behavior: schema-compiled outputs reject plain `add`; users must pass `add --schema`.
   - Future option: store schema path/hash in manifest and let plain `add` reuse it.

2. Improve catalog source ranges.
   - Track heading body line ranges accurately.
   - Attach field evidence ranges to the source range that produced each item.
   - For generated fragment pages, map source range to that fragment, not the whole source.

3. Implement FR-21 candidate priority.
   - Entity matches first.
   - Expand wikilink/backlink 1 hop from matched pages.
   - Add task field pages.
   - Add query string matches.
   - Add time metadata matches.
   - Stable sort by priority, generated path, source range start, field name, and section order.

4. Implement FR-22 budget pruning.
   - Preserve YAML frontmatter and `Source Trail`.
   - Drop optional sections from lowest priority / latest section order first.
   - Drop evidence from the end of deterministic order.
   - Replace too-large section bodies with a deterministic omitted note.

5. Add AC-31 mechanical gate.
   - Add dependency/offline check to `scripts/verify.sh` or a dedicated test.
   - Update `docs/要件定義/17-継続検証と品質スコア.md` from "planned" to actual test targets where covered.

## Open Decisions

- Schema persistence policy for `add`:
  - Current PR chooses explicit rejection for schema-compiled outputs when `add` is run without `--schema`.
  - A future manifest field could persist schema path/hash and allow plain `add` to reuse it.

- Catalog precision target:
  - Current implementation is enough for the vertical slice.
  - Full FR-19/AC-25 quality needs generated page and field evidence ranges tied to actual source sections.

## Useful Commands

```sh
rtk cargo fmt --check
rtk cargo test --test schema_context
rtk cargo test
rtk cargo clippy -- -D warnings
rtk git status --short
```
