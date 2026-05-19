# md-wiki Retrieval Compiler Progress

Last updated: 2026-05-19
Workspace: `/Users/suga/conductor/workspaces/wiki-agent/kathmandu-v1`

## Current PR Scope

Retrieval compiler implementation included in the PR branch:

- `Cargo.toml` / `Cargo.lock`: add `serde_yaml`
- `src/lib.rs`: export `schema`
- `src/output_plan.rs`: persist schema path/hash in manifest for schema-aware `add`
- `src/main.rs`: add `--schema` to `init` / `add`, add `context` subcommand
- `docs/retrieval-compiler-progress.md`: repository-visible progress tracker
- `src/schema.rs`: schema pack parsing, catalog generation, field catalogs, context pack rendering
- `tests/schema_context.rs`: acceptance coverage for schema/context vertical slice

Latest verified commands for the code changes:

- [x] `rtk proxy cargo fmt --check`
- [x] `rtk proxy cargo test --test schema_context` -> 22 passed
- [x] `rtk proxy cargo test` -> 180 passed
- [x] `rtk proxy cargo clippy -- -D warnings`
- [x] `rtk scripts/verify.sh`
  - includes `cargo fmt --check`
  - includes `cargo clippy -- -D warnings`
  - includes `cargo test`
  - includes `cargo metadata --locked --offline`
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

- [x] AC-25 catalog
  - Implemented: `.md-wiki/catalog.json` with schema id/version, generated path, source path/range, entities, tags, headings, extracted fields, outgoing links, backlinks.
  - Review fix: heading field evidence ranges now account for YAML frontmatter lines.
  - Current iteration: regular Markdown generated page source ranges now track entry / h2 fragment page line ranges, and heading field evidence is attached to the generated page containing that source range. Large Markdown pages already use `md_wiki.line_ranges`.
  - Covered by `tests/schema_context.rs::catalog_maps_regular_generated_pages_to_fragment_source_ranges`.

- [x] AC-26 field catalog
  - Covered by `tests/schema_context.rs::schema_init_generates_internal_catalog_and_field_catalogs`
  - Covered by `tests/schema_context.rs::add_with_schema_refreshes_catalog_and_field_catalogs`

- [x] AC-27 context pack generation
  - Implemented: `md-wiki context --wiki <DIR> --schema <YAML> --task <TASK>` outputs Markdown with `md_wiki_context`, recipe title, sections, and `Source Trail`.
  - Current iteration: entity matches expand wikilink/backlink 1 hop; task field pages, query matches, and time string matches are deterministically merged by priority.
  - Covered by `tests/schema_context.rs::entity_matches_expand_one_hop_links_and_backlinks` and `tests/schema_context.rs::time_filter_matches_explicit_schema_field_metadata`.

- [x] AC-28 required field missing evidence
  - Covered by `tests/schema_context.rs::context_outputs_markdown_pack_with_missing_evidence_budget_and_determinism`

- [x] AC-29 context budget
  - Implemented: output is capped by `--budget` / recipe default.
  - Review fix: budget pruning preserves YAML frontmatter, context title, and `Source Trail` marker.
  - Current iteration: budget pruning keeps required sections and `Missing Required Evidence` before optional sections when they fit, then prunes `Source Trail` deterministically.
  - Current iteration: section-internal evidence pruning keeps deterministic first evidence lines and drops later evidence before omitting the section.
  - Covered by `tests/schema_context.rs::budget_pruning_drops_evidence_from_section_end_before_omitting_section`.

- [x] AC-30 retrieval determinism
  - Covered by `tests/schema_context.rs::context_outputs_markdown_pack_with_missing_evidence_budget_and_determinism`

- [x] AC-31 no inference / no network
  - Implemented by architecture: no LLM, embedding, external API, network, or schema code execution.
  - Current iteration: `scripts/verify.sh` now includes `cargo metadata --locked --offline --format-version 1` as a dependency/offline gate.

## Next Tasks

No remaining implementation tasks for the current retrieval compiler PR scope.

Future hardening candidates, outside the current completion gate:

1. Add regular-Markdown byte ranges to catalog if downstream tools need byte-level citation.
2. Add typed time comparator semantics if schema packs later define structured time fields beyond string matching.

## Open Decisions

- Schema persistence policy for `add`:
  - Resolved: schema-compiled outputs persist schema path/hash in `.md-wiki/manifest.json`.
  - Plain `add` reuses the persisted schema when the hash matches.
  - If the schema file changed, plain `add` rejects and asks for explicit `--schema`.

## Useful Commands

```sh
rtk cargo fmt --check
rtk cargo test --test schema_context
rtk cargo test
rtk cargo clippy -- -D warnings
rtk git status --short
```
