use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;

use md_wiki::build::build_nodes;
use md_wiki::link::resolve_all;
use md_wiki::model::Node;
use md_wiki::notes::ingest_notes;
use md_wiki::relations::compute_relations;
use md_wiki::render::tags::build_tag_index;
use md_wiki::render::{WikiOutput, write_wiki};
use md_wiki::scan::{ScanConfig, scan};

const PAGE_CHAR_LIMIT: usize = 40_000;
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
struct Args {
    workspace: PathBuf,
    work_dir: PathBuf,
    history_dir: Option<PathBuf>,
    min_score: Option<f64>,
}

#[derive(Debug, Serialize)]
struct QualityReport {
    schema_version: u32,
    commit: String,
    dirty: bool,
    score: f64,
    max_score: f64,
    passed_min_score: Option<bool>,
    summary: Summary,
    checks: Vec<ScoreCheck>,
}

#[derive(Debug, Serialize)]
struct Summary {
    generated_pages: usize,
    generated_files: usize,
    broken_local_links: usize,
    unresolved_links: usize,
    oversized_pages: usize,
    max_page_chars: usize,
}

#[derive(Debug, Serialize)]
struct ScoreCheck {
    id: &'static str,
    name: &'static str,
    points: f64,
    max_points: f64,
    passed: bool,
    detail: String,
}

#[derive(Debug, Clone)]
struct GeneratedSite {
    nodes: Vec<Node>,
    unresolved_count: usize,
    output_dir: PathBuf,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    std::env::set_current_dir(&args.workspace)
        .with_context(|| format!("failed to enter {}", args.workspace.display()))?;

    let fixture_dir = args.work_dir.join("fixture");
    let out_a = args.work_dir.join("out-a");
    let out_b = args.work_dir.join("out-b");
    recreate_fixture(&fixture_dir)?;
    let first = generate_site(&fixture_dir, &out_a)?;
    let second = generate_site(&fixture_dir, &out_b)?;

    let report = build_report(&args, first, second)?;
    let json = serde_json::to_string_pretty(&report)?;
    println!("{json}");

    if let Some(history_dir) = &args.history_dir {
        write_history(history_dir, &report, &json)?;
    }

    if matches!(report.passed_min_score, Some(false)) {
        anyhow::bail!(
            "quality score {} is below required minimum {}",
            report.score,
            args.min_score.unwrap()
        );
    }

    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut workspace = PathBuf::from(".");
    let mut work_dir = PathBuf::from("target/quality-score");
    let mut history_dir = None;
    let mut min_score = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--workspace" => {
                workspace = PathBuf::from(next_value(&mut it, "--workspace")?);
            }
            "--work-dir" => {
                work_dir = PathBuf::from(next_value(&mut it, "--work-dir")?);
            }
            "--history" => {
                history_dir = Some(PathBuf::from(next_value(&mut it, "--history")?));
            }
            "--min-score" => {
                let raw = next_value(&mut it, "--min-score")?;
                min_score = Some(raw.parse::<f64>().context("--min-score must be a number")?);
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }

    Ok(Args {
        workspace,
        work_dir,
        history_dir,
        min_score,
    })
}

fn next_value(it: &mut impl Iterator<Item = String>, name: &str) -> Result<String> {
    it.next()
        .with_context(|| format!("{name} requires a value"))
}

fn print_help() {
    println!(
        "quality_score\n\n\
         Usage:\n\
           cargo run --example quality_score -- [--history DIR] [--min-score N]\n\n\
         Options:\n\
           --workspace DIR   Repository root. Default: .\n\
           --work-dir DIR    Temporary fixture/output dir. Default: target/quality-score\n\
           --history DIR     Write <commit>.json, <commit>.md, latest.json, latest.md\n\
           --min-score N     Exit non-zero if score is below N\n"
    );
}

fn recreate_fixture(root: &Path) -> Result<()> {
    if root.exists() {
        fs::remove_dir_all(root).with_context(|| format!("failed to clear {}", root.display()))?;
    }
    fs::create_dir_all(root.join("deep"))
        .with_context(|| format!("failed to create {}", root.display()))?;

    fs::write(
        root.join("overview.md"),
        r#"---
title: Overview
summary: Entry point for the quality fixture
tags: [product/wiki, quality/check]
related: [deep-dive]
aliases: [HomeNote]
---
# Overview

This fixture links to [[deep-dive#Design|the design section]] and [[nested]].

## Intro

The intro references [[deep-dive]].

## Details

The details point back to [[HomeNote]].
"#,
    )?;

    fs::write(
        root.join("deep-dive.md"),
        r#"---
title: Deep Dive
summary: Design and operations detail
tags: [product/wiki, quality/check, operations/runbook]
aliases: [deep-dive]
---
# Deep Dive

## Design

The design links to [[overview]].

## Operations

The operations section links to [[nested]].
"#,
    )?;

    fs::write(
        root.join("plain.md"),
        r#"---
title: Plain
fragment: false
tags: [quality/check]
---
# Plain

## Kept Together

This note opts out of fragmentation but still links to [[overview]].
"#,
    )?;

    fs::write(
        root.join("deep/nested.md"),
        r#"---
title: Nested
tags: [product/wiki, nested/topic]
---
# Nested

Nested content links to [[deep-dive#Operations]].
"#,
    )?;

    let mut long = String::from(
        "---\ntitle: Long Form\ntags: [product/wiki, quality/check]\n---\n# Long Form\n\n## Research\n\n",
    );
    long.push_str("### Alpha\n");
    for i in 0..155 {
        long.push_str(&format!("alpha line {i}\n"));
    }
    long.push_str("\n### Beta\n");
    for i in 0..155 {
        long.push_str(&format!("beta line {i}\n"));
    }
    fs::write(root.join("long-form.md"), long)?;

    Ok(())
}

fn generate_site(input: &Path, output: &Path) -> Result<GeneratedSite> {
    if output.exists() {
        fs::remove_dir_all(output)
            .with_context(|| format!("failed to clear {}", output.display()))?;
    }

    let files = scan(&ScanConfig {
        root: input.to_path_buf(),
        extra_excluded: vec![output.to_path_buf()],
        recursive: true,
    });
    let notes = ingest_notes(&files, input);
    let mut nodes = build_nodes(notes);
    let (unresolved, graph) = resolve_all(&nodes);
    let tag_index = build_tag_index(&nodes);
    compute_relations(&mut nodes, &graph, &tag_index);
    write_wiki(
        output,
        &WikiOutput {
            project_title: "quality-fixture",
            nodes: &nodes,
            unresolved: &unresolved,
            graph: &graph,
        },
    )?;

    Ok(GeneratedSite {
        nodes,
        unresolved_count: unresolved.len(),
        output_dir: output.to_path_buf(),
    })
}

fn build_report(args: &Args, first: GeneratedSite, second: GeneratedSite) -> Result<QualityReport> {
    let pages = markdown_files(&first.output_dir)?;
    let generated_files = all_files(&first.output_dir)?.len();
    let broken_local_links = count_broken_local_links(&first.output_dir, &pages)?;
    let oversized_pages = pages
        .values()
        .filter(|body| body.chars().count() > PAGE_CHAR_LIMIT)
        .count();
    let max_page_chars = pages
        .values()
        .map(|body| body.chars().count())
        .max()
        .unwrap_or(0);
    let deterministic = snapshot_dir(&first.output_dir)? == snapshot_dir(&second.output_dir)?;

    let checks = vec![
        required_files_check(&pages),
        local_links_check(broken_local_links),
        wikilink_rewrite_check(&pages, first.unresolved_count),
        navigation_check(&pages, &first.nodes),
        backlinks_related_check(&pages),
        context_efficiency_check(oversized_pages, max_page_chars),
        determinism_check(deterministic),
    ];

    let score: f64 = checks.iter().map(|c| c.points).sum();
    let max_score: f64 = checks.iter().map(|c| c.max_points).sum();
    let passed_min_score = args.min_score.map(|min| score >= min);

    Ok(QualityReport {
        schema_version: SCHEMA_VERSION,
        commit: git_output(["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "unknown".into()),
        dirty: git_output(["status", "--porcelain"])
            .map(|s| !s.trim().is_empty())
            .unwrap_or(true),
        score,
        max_score,
        passed_min_score,
        summary: Summary {
            generated_pages: pages.len(),
            generated_files,
            broken_local_links,
            unresolved_links: first.unresolved_count,
            oversized_pages,
            max_page_chars,
        },
        checks,
    })
}

fn required_files_check(pages: &BTreeMap<PathBuf, String>) -> ScoreCheck {
    let required = [
        Path::new("index.md"),
        Path::new("fragments/_index.md"),
        Path::new("tags/index.md"),
        Path::new("headings/index.md"),
        Path::new("links/index.md"),
        Path::new("_unresolved.md"),
    ];
    let missing: Vec<_> = required
        .iter()
        .filter(|p| !pages.contains_key(**p))
        .map(|p| p.display().to_string())
        .collect();
    let passed = missing.is_empty();
    ScoreCheck {
        id: "required-files",
        name: "Required output entry points",
        points: if passed { 15.0 } else { 0.0 },
        max_points: 15.0,
        passed,
        detail: if passed {
            "all required entry points exist".into()
        } else {
            format!("missing: {}", missing.join(", "))
        },
    }
}

fn local_links_check(broken: usize) -> ScoreCheck {
    let passed = broken == 0;
    ScoreCheck {
        id: "local-links",
        name: "Generated local links resolve",
        points: if passed { 20.0 } else { 0.0 },
        max_points: 20.0,
        passed,
        detail: format!("broken local links: {broken}"),
    }
}

fn wikilink_rewrite_check(
    pages: &BTreeMap<PathBuf, String>,
    unresolved_count: usize,
) -> ScoreCheck {
    let raw_wikilinks = pages
        .iter()
        .filter(|(path, _)| path.as_path() != Path::new("_unresolved.md"))
        .flat_map(|(_, body)| body.match_indices("[["))
        .count();
    let has_expected_link = pages
        .get(Path::new("fragments/overview/details.md"))
        .is_some_and(|body| body.contains("[HomeNote](index.md)"));
    let passed = raw_wikilinks == 0 && unresolved_count == 0 && has_expected_link;
    ScoreCheck {
        id: "wikilink-rewrite",
        name: "Wikilinks are rewritten without residue",
        points: if passed { 15.0 } else { 0.0 },
        max_points: 15.0,
        passed,
        detail: format!(
            "raw wikilinks: {raw_wikilinks}, unresolved: {unresolved_count}, alias link ok: {has_expected_link}"
        ),
    }
}

fn navigation_check(pages: &BTreeMap<PathBuf, String>, nodes: &[Node]) -> ScoreCheck {
    let expected = expected_nav_pages(nodes);
    let failed: Vec<_> = expected
        .iter()
        .filter(|path| {
            pages
                .get(*path)
                .map(|body| {
                    let mut lines = body
                        .lines()
                        .skip_while(|line| !line.starts_with("> Parent: "));
                    !matches!(lines.next(), Some(line) if line.starts_with("> Parent: "))
                        || !matches!(lines.next(), Some("---"))
                })
                .unwrap_or(true)
        })
        .map(|p| p.display().to_string())
        .collect();
    let passed = failed.is_empty() && !expected.is_empty();
    ScoreCheck {
        id: "fragment-navigation",
        name: "Fragment navigation is present and stable",
        points: if passed { 15.0 } else { 0.0 },
        max_points: 15.0,
        passed,
        detail: if passed {
            format!("checked {} navigable pages", expected.len())
        } else {
            format!("navigation failures: {}", failed.join(", "))
        },
    }
}

fn backlinks_related_check(pages: &BTreeMap<PathBuf, String>) -> ScoreCheck {
    let backlink_ok = pages
        .get(Path::new("fragments/deep-dive/design.md"))
        .is_some_and(|body| {
            body.contains("## Backlinks") && body.contains("[Overview](../overview/index.md)")
        });
    let related_ok = pages
        .get(Path::new("fragments/overview/index.md"))
        .is_some_and(|body| {
            body.contains("## Related") && body.contains("[Deep Dive](../deep-dive/index.md)")
        });
    let passed = backlink_ok && related_ok;
    ScoreCheck {
        id: "relations",
        name: "Backlinks and related notes are rendered",
        points: match (backlink_ok, related_ok) {
            (true, true) => 15.0,
            (true, false) | (false, true) => 7.5,
            (false, false) => 0.0,
        },
        max_points: 15.0,
        passed,
        detail: format!("backlink ok: {backlink_ok}, related ok: {related_ok}"),
    }
}

fn context_efficiency_check(oversized: usize, max_chars: usize) -> ScoreCheck {
    let passed = oversized == 0;
    ScoreCheck {
        id: "context-efficiency",
        name: "Generated pages stay within context budget",
        points: if passed { 10.0 } else { 0.0 },
        max_points: 10.0,
        passed,
        detail: format!("oversized pages: {oversized}, max chars: {max_chars}"),
    }
}

fn determinism_check(deterministic: bool) -> ScoreCheck {
    ScoreCheck {
        id: "determinism",
        name: "Repeated generation is byte-identical",
        points: if deterministic { 10.0 } else { 0.0 },
        max_points: 10.0,
        passed: deterministic,
        detail: format!("byte-identical rerun: {deterministic}"),
    }
}

fn expected_nav_pages(nodes: &[Node]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for node in nodes {
        for frag in &node.fragments.fragments {
            match frag {
                md_wiki::fragment::Fragment::H2 { slug, .. } => {
                    out.push(md_wiki::render::paths::fragment_leaf_path(
                        &node.entry_dir,
                        slug,
                    ));
                }
                md_wiki::fragment::Fragment::Shell { slug, children, .. } => {
                    out.push(md_wiki::render::paths::shell_index_path(
                        &node.entry_dir,
                        slug,
                    ));
                    for child in children {
                        out.push(md_wiki::render::paths::h3_leaf_path(
                            &node.entry_dir,
                            slug,
                            &child.slug,
                        ));
                    }
                }
            }
        }
    }
    out
}

fn markdown_files(root: &Path) -> Result<BTreeMap<PathBuf, String>> {
    let mut out = BTreeMap::new();
    for path in all_files(root)? {
        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            let rel = path.strip_prefix(root)?.to_path_buf();
            let body = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            out.insert(rel, body);
        }
    }
    Ok(out)
}

fn all_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let ty = entry.file_type()?;
            if ty.is_dir() {
                stack.push(path);
            } else if ty.is_file() {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn snapshot_dir(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut out = BTreeMap::new();
    for path in all_files(root)? {
        let rel = path.strip_prefix(root)?.to_path_buf();
        out.insert(rel, fs::read(path)?);
    }
    Ok(out)
}

fn count_broken_local_links(root: &Path, pages: &BTreeMap<PathBuf, String>) -> Result<usize> {
    let re = Regex::new(r"\[[^\]\n]+\]\(([^)\s]+)\)")?;
    let mut broken = 0;
    for (rel, body) in pages {
        let from_dir = rel.parent().unwrap_or(Path::new(""));
        for cap in re.captures_iter(body) {
            let target = &cap[1];
            if target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
                continue;
            }
            let path_part = target.split('#').next().unwrap_or(target);
            if path_part.is_empty() {
                continue;
            }
            let normalized = normalize_relative(from_dir, Path::new(path_part));
            if !root.join(normalized).exists() {
                broken += 1;
            }
        }
    }
    Ok(broken)
}

fn normalize_relative(base: &Path, rel: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in base.join(rel).components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            Component::Normal(s) => out.push(s),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    out
}

fn write_history(history_dir: &Path, report: &QualityReport, json: &str) -> Result<()> {
    fs::create_dir_all(history_dir)
        .with_context(|| format!("failed to create {}", history_dir.display()))?;
    let stem = if report.dirty {
        format!("{}-dirty", report.commit)
    } else {
        report.commit.clone()
    };
    fs::write(history_dir.join(format!("{stem}.json")), json)?;
    fs::write(
        history_dir.join(format!("{stem}.md")),
        render_markdown_report(report),
    )?;
    fs::write(history_dir.join("latest.json"), json)?;
    fs::write(
        history_dir.join("latest.md"),
        render_markdown_report(report),
    )?;
    Ok(())
}

fn render_markdown_report(report: &QualityReport) -> String {
    let mut out = String::new();
    out.push_str("# md-wiki Quality Score\n\n");
    out.push_str(&format!("- Commit: `{}`\n", report.commit));
    out.push_str(&format!("- Dirty working tree: `{}`\n", report.dirty));
    out.push_str(&format!(
        "- Score: `{:.1}/{:.1}`\n",
        report.score, report.max_score
    ));
    if let Some(passed) = report.passed_min_score {
        out.push_str(&format!("- Passed minimum score: `{passed}`\n"));
    }
    out.push_str(&format!(
        "- Generated pages: `{}`\n",
        report.summary.generated_pages
    ));
    out.push_str(&format!(
        "- Broken local links: `{}`\n",
        report.summary.broken_local_links
    ));
    out.push_str(&format!(
        "- Unresolved links: `{}`\n",
        report.summary.unresolved_links
    ));
    out.push_str(&format!(
        "- Oversized pages: `{}`\n\n",
        report.summary.oversized_pages
    ));
    out.push_str("## Checks\n\n");
    out.push_str("| ID | Points | Status | Detail |\n");
    out.push_str("|---|---:|---|---|\n");
    for check in &report.checks {
        out.push_str(&format!(
            "| `{}` | {:.1}/{:.1} | {} | {} |\n",
            check.id,
            check.points,
            check.max_points,
            if check.passed { "pass" } else { "fail" },
            check.detail.replace('|', "\\|")
        ));
    }
    out
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
