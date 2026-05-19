//! Retrieval compiler schema/context acceptance tests.

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn bin_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_md-wiki"))
}

fn run(args: &[&std::ffi::OsStr]) -> std::process::Output {
    let output = Command::new(bin_path()).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "md-wiki failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn run_fail(args: &[&std::ffi::OsStr]) -> std::process::Output {
    let output = Command::new(bin_path()).args(args).output().unwrap();
    assert!(
        !output.status.success(),
        "md-wiki unexpectedly succeeded: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn write_schema(path: &Path) {
    fs::write(
        path,
        r#"id: game-narrative
version: 1
fields:
  canon:
    label: Canon
    sources:
      - frontmatter: canon
      - heading: Canon
  setup:
    label: Setup
    sources:
      - frontmatter: narrative.setup
      - heading: Setup
  do_not_break:
    label: Constraints
    sources:
      - frontmatter: do_not_break
      - heading: Do Not Break
contexts:
  twist-payoff:
    title: Twist / Payoff Context
    default_budget_chars: 2000
    sections:
      - title: Hard Canon
        fields: [canon]
        required: true
      - title: Setup Already Planted
        fields: [setup]
      - title: Constraints
        fields: [do_not_break]
      - title: Source Trail
        kind: sources
"#,
    )
    .unwrap();
}

fn large_markdown_body() -> String {
    let line = format!("{}\n", "large markdown content ".repeat(40));
    let mut body = String::from("# Huge\n\n");
    while body.len() <= 1024 * 1024 + 4096 {
        body.push_str(&line);
    }
    body
}

#[test]
fn schema_init_generates_internal_catalog_and_field_catalogs() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        r#"---
title: A
tags: [plot]
aliases: [hero-a]
canon: A is alive.
narrative:
  setup: A kept the broken key.
---
# A

## Canon

A cannot leave town.

## Setup

The key is visible in chapter 1.
"#,
    )
    .unwrap();
    fs::write(input.join("b.md"), "# B\n\nlinks [[A]]\n").unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");

    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join(".md-wiki/catalog.json")).unwrap()).unwrap();
    assert_eq!(catalog["schema"]["id"], "game-narrative");
    assert_eq!(catalog["schema"]["version"], 1);
    let pages = catalog["pages"].as_array().unwrap();
    assert!(pages.iter().any(|page| {
        page["generated_path"] == "fragments/a/index.md"
            && page["fields"]["canon"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
    }));

    let field_index = fs::read_to_string(out.join("agent/fields/index.md")).unwrap();
    assert!(field_index.contains("[Canon](canon.md)"));
    let canon = fs::read_to_string(out.join("agent/fields/canon.md")).unwrap();
    assert!(canon.contains("fragments/a/index.md"));
    assert!(canon.contains("A is alive."));
    let agent_guide = fs::read_to_string(out.join("agent/index.md")).unwrap();
    assert!(agent_guide.contains("fields/index.md"));
    let page_catalog = fs::read_to_string(out.join("agent/pages/index.md")).unwrap();
    assert!(page_catalog.contains("agent/fields/canon.md"));
}

#[test]
fn schema_catalog_includes_large_markdown_pages() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("huge.md"), large_markdown_body()).unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");

    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join(".md-wiki/catalog.json")).unwrap()).unwrap();
    let pages = catalog["pages"].as_array().unwrap();
    assert!(
        pages
            .iter()
            .any(|page| page["generated_path"] == "fragments/huge/index.md"),
        "{catalog:#}"
    );
    let leaf = pages
        .iter()
        .find(|page| page["generated_path"] == "fragments/huge/part-001.md")
        .expect("large Markdown leaf should be cataloged");
    assert_eq!(leaf["source_path"], "huge.md");
    assert_eq!(leaf["title"], "Part 1");
    assert_eq!(leaf["source_range"]["line_start"], 1);
    assert!(leaf["source_range"]["line_end"].as_u64().unwrap() >= 1);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--query".as_ref(),
        "Part 1".as_ref(),
        "--budget".as_ref(),
        "1000".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.contains("fragments/huge/part-001.md"), "{pack}");
    assert!(pack.contains("huge.md"), "{pack}");
}

#[test]
fn schema_validation_rejects_undefined_context_fields() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();
    let schema = tmp.path().join("bad.yml");
    fs::write(
        &schema,
        r#"id: bad
version: 1
fields:
  canon:
    sources:
      - heading: Canon
contexts:
  task:
    sections:
      - title: Missing
        fields: [unknown]
"#,
    )
    .unwrap();
    let out = tmp.path().join("wiki");

    let output = run_fail(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("undefined field"));
}

#[test]
fn schema_validation_rejects_malformed_yaml_and_missing_required_top_level_fields() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();
    let out = tmp.path().join("wiki");

    let malformed = tmp.path().join("malformed.yml");
    fs::write(&malformed, "id: bad\nfields:\n  canon: [").unwrap();
    let output = run_fail(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        malformed.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to parse"), "{stderr}");

    let missing = tmp.path().join("missing.yml");
    fs::write(
        &missing,
        r#"id: missing
version: 1
fields:
  canon:
    sources:
      - heading: Canon
"#,
    )
    .unwrap();
    let output = run_fail(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        missing.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema contexts are required"), "{stderr}");
}

#[test]
fn context_outputs_markdown_pack_with_missing_evidence_budget_and_determinism() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        r#"---
title: A
tags: [plot]
aliases: [hero-a]
narrative:
  setup: A kept the broken key.
---
# A

## Setup

The key is visible in chapter 1.
"#,
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let args = [
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--entity".as_ref(),
        "hero-a".as_ref(),
        "--query".as_ref(),
        "broken key".as_ref(),
        "--budget".as_ref(),
        "900".as_ref(),
    ];
    let first = run(&args);
    let second = run(&args);
    assert_eq!(first.stdout, second.stdout);
    assert!(first.stdout.len() <= 900);

    let pack = String::from_utf8(first.stdout).unwrap();
    assert!(pack.starts_with("---\nmd_wiki_context:\n"));
    assert!(pack.contains("# Twist / Payoff Context"));
    assert!(pack.contains("## Setup Already Planted"));
    assert!(pack.contains("A kept the broken key."));
    assert!(pack.contains("## Missing Required Evidence"));
    assert!(pack.contains("canon"));
    assert!(pack.contains("## Source Trail"));
    assert!(pack.contains("fragments/a/index.md"));
}

#[test]
fn entity_matched_page_with_non_recipe_fields_stays_in_source_trail() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        r#"---
title: A
aliases: [hero-a]
lore: A carries outside-recipe lore.
---
# A
"#,
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    fs::write(
        &schema,
        r#"id: game-narrative
version: 1
fields:
  canon:
    sources:
      - frontmatter: canon
  lore:
    sources:
      - frontmatter: lore
contexts:
  canon-only:
    sections:
      - title: Canon
        fields: [canon]
      - title: Source Trail
        kind: sources
"#,
    )
    .unwrap();
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "canon-only".as_ref(),
        "--entity".as_ref(),
        "hero-a".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.contains("## Source Trail"), "{pack}");
    assert!(pack.contains("fragments/a/index.md"), "{pack}");
    assert!(!pack.contains("No sources matched"), "{pack}");
}

#[test]
fn context_without_entity_does_not_select_unrelated_pages() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "---\ncanon: A is fixed.\n---\n# A\n").unwrap();
    fs::write(input.join("b.md"), "# B\n\nUnrelated text.\n").unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.contains("fragments/a/index.md"), "{pack}");
    assert!(!pack.contains("fragments/b/index.md"), "{pack}");
}

#[test]
fn time_filter_matches_explicit_schema_field_metadata() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        "---\ntimeline: after:chapter-3\n---\n# A\n",
    )
    .unwrap();
    fs::write(
        input.join("b.md"),
        "---\ntimeline: before:chapter-1\n---\n# B\n",
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    fs::write(
        &schema,
        r#"id: time-test
version: 1
fields:
  timeline:
    sources:
      - frontmatter: timeline
contexts:
  route:
    sections:
      - title: Source Trail
        kind: sources
"#,
    )
    .unwrap();
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "route".as_ref(),
        "--time".as_ref(),
        "after:chapter-3".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.contains("fragments/a/index.md"), "{pack}");
    assert!(!pack.contains("fragments/b/index.md"), "{pack}");
}

#[test]
fn required_evidence_is_scoped_to_requested_entity_candidates() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        r#"---
title: A
aliases: [hero-a]
---
# A
"#,
    )
    .unwrap();
    fs::write(input.join("b.md"), "---\ncanon: B has canon.\n---\n# B\n").unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--entity".as_ref(),
        "hero-a".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.contains("## Missing Required Evidence"), "{pack}");
    assert!(pack.contains("canon"), "{pack}");
}

#[test]
fn entity_matches_expand_one_hop_links_and_backlinks() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        r#"---
title: A
aliases: [hero-a]
---
# A

See [[support]].
"#,
    )
    .unwrap();
    fs::write(input.join("support.md"), "# Support\n\nSupporting page.\n").unwrap();
    fs::write(
        input.join("witness.md"),
        "# Witness\n\nLinks back to [[A]].\n",
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--entity".as_ref(),
        "hero-a".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.contains("fragments/a/index.md"), "{pack}");
    assert!(pack.contains("fragments/support/index.md"), "{pack}");
    assert!(pack.contains("fragments/witness/index.md"), "{pack}");
}

#[test]
fn field_evidence_uses_source_appearance_order_not_schema_source_order() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        r#"---
canon: Frontmatter canon.
---
# A

## Canon

Heading canon.
"#,
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    fs::write(
        &schema,
        r#"id: order-test
version: 1
fields:
  canon:
    sources:
      - heading: Canon
      - frontmatter: canon
contexts:
  canon:
    sections:
      - title: Canon
        fields: [canon]
"#,
    )
    .unwrap();
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join(".md-wiki/catalog.json")).unwrap()).unwrap();
    let entry = catalog["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["generated_path"] == "fragments/a/index.md")
        .unwrap();
    let leaf = catalog["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["generated_path"] == "fragments/a/canon.md")
        .unwrap();
    assert_eq!(entry["fields"]["canon"][0]["text"], "Frontmatter canon.");
    assert_eq!(leaf["fields"]["canon"][0]["text"], "Heading canon.");
    assert!(
        entry["fields"]["canon"][0]["line_start"].as_u64().unwrap()
            < leaf["fields"]["canon"][0]["line_start"].as_u64().unwrap()
    );
}

#[test]
fn budget_pruning_preserves_missing_required_evidence() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    let long_setup = "A very long planted setup. ".repeat(80);
    fs::write(
        input.join("a.md"),
        format!("---\nnarrative:\n  setup: {long_setup:?}\n---\n# A\n"),
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--budget".as_ref(),
        "320".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.len() <= 320, "{pack}");
    assert!(pack.contains("## Missing Required Evidence"), "{pack}");
    assert!(pack.contains("canon"), "{pack}");
    assert!(pack.contains("## Source Trail"), "{pack}");
}

#[test]
fn budget_pruning_keeps_required_sections_before_optional_sections() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    let long_setup = "optional setup evidence ".repeat(80);
    fs::write(
        input.join("a.md"),
        format!("---\ncanon: Required canon.\nnarrative:\n  setup: {long_setup:?}\n---\n# A\n"),
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--budget".as_ref(),
        "520".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.len() <= 520, "{pack}");
    assert!(pack.contains("## Hard Canon"), "{pack}");
    assert!(pack.contains("Required canon."), "{pack}");
    assert!(
        !pack.contains("optional setup evidence optional setup evidence"),
        "{pack}"
    );
    assert!(pack.contains("## Source Trail"), "{pack}");
}

#[test]
fn budget_pruning_drops_evidence_from_section_end_before_omitting_section() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    for idx in 0..8 {
        fs::write(
            input.join(format!("canon-{idx}.md")),
            format!("---\ncanon: Canon evidence {idx}.\n---\n# Canon {idx}\n"),
        )
        .unwrap();
    }
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--budget".as_ref(),
        "620".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.len() <= 620, "{pack}");
    assert!(pack.contains("## Hard Canon"), "{pack}");
    assert!(pack.contains("Canon evidence 0."), "{pack}");
    assert!(!pack.contains("Canon evidence 7."), "{pack}");
    assert!(
        pack.contains("Omitted due to budget; see Source Trail."),
        "{pack}"
    );
    assert!(pack.contains("## Source Trail"), "{pack}");
}

#[test]
fn context_frontmatter_escapes_arbitrary_cli_values() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        "---\ntitle: A\ncanon: A says \"yes\".\n---\n# A\n",
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--entity".as_ref(),
        "hero\"a\\b".as_ref(),
        "--query".as_ref(),
        "quote \" and slash \\ test".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    let fm_end = pack[4..].find("\n---\n").unwrap() + 4;
    let frontmatter = &pack[4..fm_end];
    let parsed: serde_yaml::Value = serde_yaml::from_str(frontmatter).unwrap();
    assert_eq!(
        parsed["md_wiki_context"]["query"].as_str(),
        Some("quote \" and slash \\ test")
    );
    assert_eq!(
        parsed["md_wiki_context"]["entities"][0].as_str(),
        Some("hero\"a\\b")
    );
}

#[test]
fn context_budget_preserves_frontmatter_and_source_trail() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    let mut note = String::from("---\ntitle: A\ncanon: A is fixed.\n---\n# A\n\n");
    for idx in 0..20 {
        note.push_str(&format!("## Canon\n\nEvidence item {idx}.\n\n"));
    }
    fs::write(input.join("a.md"), note).unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--budget".as_ref(),
        "220".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.len() <= 220, "{pack}");
    assert!(pack.starts_with("---\nmd_wiki_context:\n"), "{pack}");
    assert!(pack.contains("# Twist / Payoff Context"), "{pack}");
    assert!(pack.contains("## Source Trail"), "{pack}");
}

#[test]
fn context_respects_budget_below_200() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "---\ncanon: A is fixed.\n---\n# A\n").unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let output = run(&[
        "context".as_ref(),
        "--wiki".as_ref(),
        out.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--task".as_ref(),
        "twist-payoff".as_ref(),
        "--budget".as_ref(),
        "100".as_ref(),
    ]);
    let pack = String::from_utf8(output.stdout).unwrap();
    assert!(pack.len() <= 100, "{pack}");
    assert!(pack.starts_with("---\nmd_wiki_context:\n"), "{pack}");
    assert!(pack.contains("## Source Trail"), "{pack}");
}

#[test]
fn heading_field_source_ranges_include_frontmatter_lines() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        r#"---
title: A
tags: [plot]
aliases: [hero-a]
---
# A

## Setup

The key is visible in chapter 1.
"#,
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join(".md-wiki/catalog.json")).unwrap()).unwrap();
    let setup = &catalog["pages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|page| page["generated_path"] == "fragments/a/setup.md")
        .unwrap()["fields"]["setup"][0];
    assert_eq!(setup["line_start"], 10);
    assert_eq!(setup["line_end"], 10);
}

#[test]
fn catalog_maps_regular_generated_pages_to_fragment_source_ranges() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        r#"---
title: A
canon: Frontmatter canon.
---
# A

Preface line.

## Canon

Heading canon.

## Setup

Setup evidence.
"#,
    )
    .unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join(".md-wiki/catalog.json")).unwrap()).unwrap();
    let page = |path: &str| {
        catalog["pages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|page| page["generated_path"] == path)
            .unwrap_or_else(|| panic!("missing catalog page {path}"))
    };

    assert_eq!(
        page("fragments/a/index.md")["source_range"]["line_start"],
        1
    );
    assert_eq!(page("fragments/a/index.md")["source_range"]["line_end"], 8);
    assert_eq!(
        page("fragments/a/canon.md")["source_range"]["line_start"],
        9
    );
    assert_eq!(page("fragments/a/canon.md")["source_range"]["line_end"], 12);
    assert_eq!(
        page("fragments/a/setup.md")["source_range"]["line_start"],
        13
    );
    assert_eq!(page("fragments/a/setup.md")["source_range"]["line_end"], 15);

    assert!(
        page("fragments/a/index.md")["fields"]["canon"]
            .as_array()
            .is_some_and(|items| items
                .iter()
                .any(|item| item["text"] == "Frontmatter canon."))
    );
    assert!(
        page("fragments/a/canon.md")["fields"]["canon"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["text"] == "Heading canon."))
    );
    assert!(
        page("fragments/a/setup.md")["fields"]["setup"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["text"] == "Setup evidence."))
    );
}

#[test]
fn add_with_schema_refreshes_catalog_and_field_catalogs() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A\n").unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    fs::write(
        input.join("b.md"),
        r#"---
title: B
canon: B remembers A.
---
# B
"#,
    )
    .unwrap();
    run(&[
        "add".as_ref(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join(".md-wiki/catalog.json")).unwrap()).unwrap();
    assert!(catalog["pages"].as_array().unwrap().iter().any(|page| {
        page["generated_path"] == "fragments/b/index.md"
            && page["fields"]["canon"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
    }));
    let canon = fs::read_to_string(out.join("agent/fields/canon.md")).unwrap();
    assert!(canon.contains("B remembers A."));
}

#[test]
fn plain_add_after_schema_init_reuses_persisted_schema() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "---\ncanon: A.\n---\n# A\n").unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join(".md-wiki/manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["schema"]["path"],
        schema
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    );
    assert!(manifest["schema"]["hash"].as_str().is_some());

    fs::write(input.join("b.md"), "---\ncanon: B.\n---\n# B\n").unwrap();
    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);
    assert!(out.join(".md-wiki/catalog.json").exists());
    assert!(out.join("agent/fields/canon.md").exists());
    let canon = fs::read_to_string(out.join("agent/fields/canon.md")).unwrap();
    assert!(canon.contains("B."), "{canon}");
}

#[test]
fn plain_add_rejects_changed_persisted_schema() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "---\ncanon: A.\n---\n# A\n").unwrap();
    let schema = tmp.path().join("schema.yml");
    write_schema(&schema);
    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--schema".as_ref(),
        schema.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let mut body = fs::read_to_string(&schema).unwrap();
    body.push_str("\n# changed\n");
    fs::write(&schema, body).unwrap();
    fs::write(input.join("b.md"), "# B\n").unwrap();

    let output = run_fail(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema pack changed"), "{stderr}");
    assert!(stderr.contains("--schema"), "{stderr}");
}
