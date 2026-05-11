use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Serialize;

use crate::agentic_output::{finalize_agentic_output, write_large_markdown_pages};
use crate::build::build_nodes;
use crate::input_classifier::{InputKind, classify_scanned};
use crate::link::resolve_all;
use crate::metadata_renderer::markdown_path;
use crate::notes::ingest_notes;
use crate::relations::compute_relations;
use crate::render::tags::build_tag_index;
use crate::render::{WikiOutput, write_wiki};
use crate::scan::{ScanConfig, scan};

const SCHEMA_VERSION: u32 = 1;
const DEFAULT_NORMAL_FIXTURE_BYTES: usize = 1024 * 1024 + 64 * 1024;
const DEFAULT_HEAVY_FIXTURE_BYTES: usize = 3 * 1024 * 1024;
const EXPECTED_UNRESOLVED_LINKS: usize = 1;

pub const CHECK_IDS: &[&str] = &[
    "agentic-entrypoint",
    "agentic-page-budget",
    "agentic-metadata",
    "agentic-catalog",
    "agentic-term-index",
    "agentic-navigation",
    "agentic-routing",
    "agentic-query-fixtures",
    "agentic-traceability",
    "agentic-link-graph",
    "agentic-no-full-read",
    "agentic-determinism",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateMode {
    Normal,
    Heavy,
}

#[derive(Debug, Clone)]
pub struct GateOptions {
    pub mode: GateMode,
    pub work_dir: PathBuf,
    pub report_path: Option<PathBuf>,
    pub min_score: Option<f64>,
    pub require_resource_budget: bool,
    pub fixture_bytes_override: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct GateReport {
    pub schema_version: u32,
    pub mode: GateMode,
    pub passed: bool,
    pub score: f64,
    pub max_score: f64,
    pub summary: GateSummary,
    pub query_simulations: Vec<QuerySimulation>,
    pub checks: Vec<GateCheck>,
}

#[derive(Debug, Serialize)]
pub struct GateSummary {
    pub fixtures: usize,
    pub input_bytes: u64,
    pub generated_pages: usize,
    pub max_page_chars: usize,
    pub oversized_pages: usize,
    pub broken_local_links: usize,
    pub unresolved_links: usize,
    pub expected_unresolved_links: usize,
    pub input_hash_changed: bool,
    pub byte_identical_rerun: bool,
    pub peak_rss_bytes: Option<u64>,
    pub elapsed_ms: u128,
    pub max_buffered_bytes: usize,
}

#[derive(Debug, Serialize)]
pub struct GateCheck {
    pub id: &'static str,
    pub passed: bool,
    pub points: f64,
    pub max_points: f64,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct QuerySimulation {
    pub query: &'static str,
    pub route: Vec<PathBuf>,
    pub expected_page: PathBuf,
    pub read_count: usize,
    pub reached: bool,
}

pub fn run_gate(options: &GateOptions) -> Result<GateReport> {
    let started = Instant::now();
    if options.work_dir.exists() {
        fs::remove_dir_all(&options.work_dir)
            .with_context(|| format!("failed to clear {}", options.work_dir.display()))?;
    }
    fs::create_dir_all(&options.work_dir)
        .with_context(|| format!("failed to create {}", options.work_dir.display()))?;

    let fixture_dir = options.work_dir.join("fixture");
    fs::create_dir_all(&fixture_dir)?;
    let fixture_bytes = fixture_bytes(options);
    let fixtures = write_fixtures(&fixture_dir, fixture_bytes)?;
    let input_before = snapshot_dir(&fixture_dir)?;

    let out_a = options.work_dir.join("out-a");
    let out_b = options.work_dir.join("out-b");
    generate_wiki(&fixture_dir, &out_a)?;
    generate_wiki(&fixture_dir, &out_b)?;

    let input_after = snapshot_dir(&fixture_dir)?;
    let input_hash_changed = input_before != input_after;
    let snapshot_a = snapshot_dir(&out_a)?;
    let snapshot_b = snapshot_dir(&out_b)?;
    let byte_identical_rerun = snapshot_a == snapshot_b;
    let pages = markdown_files(&out_a)?;
    let generated_pages = pages.len();
    let max_page_chars = pages
        .iter()
        .map(|(_, body)| body.chars().count())
        .max()
        .unwrap_or(0);
    let oversized_pages = pages
        .iter()
        .filter(|(_, body)| body.chars().count() > 40_000)
        .count();
    let broken_local_links = count_broken_local_links(&out_a, &pages)?;
    let unresolved_links = count_unresolved_links(&pages);
    let query_simulations = simulate_agent_queries(&out_a);
    let input_bytes = fixtures
        .iter()
        .map(|path| fs::metadata(path).map(|m| m.len()).unwrap_or(0))
        .sum();

    let checks = vec![
        scored_check(
            "agentic-entrypoint",
            validate_entrypoint(&out_a, &pages),
            "root index reaches agent guide and core indexes".into(),
        ),
        scored_check(
            "agentic-page-budget",
            oversized_pages == 0 && generated_pages > 0,
            format!("oversized pages: {oversized_pages}, max chars: {max_page_chars}"),
        ),
        scored_check(
            "agentic-metadata",
            validate_metadata(&pages),
            "all Markdown pages expose md_wiki frontmatter and page_kind".into(),
        ),
        scored_check(
            "agentic-catalog",
            validate_catalog(&out_a, &pages)?,
            "agent/pages catalog lists generated pages without missing paths".into(),
        ),
        scored_check(
            "agentic-term-index",
            validate_term_index(&out_a),
            "agent/terms contains deterministic heading, tag, file, and link terms".into(),
        ),
        scored_check(
            "agentic-navigation",
            validate_navigation(&out_a, &pages)?,
            "Parent/Prev/Next/Children links resolve locally".into(),
        ),
        scored_check(
            "agentic-routing",
            validate_routing(&out_a),
            "agent guide includes primary query routes".into(),
        ),
        scored_check(
            "agentic-query-fixtures",
            validate_query_fixtures(&out_a)
                && query_simulations
                    .iter()
                    .all(|simulation| simulation.reached && simulation.read_count <= 5),
            "representative queries can reach expected pages within bounded read routes".into(),
        ),
        scored_check(
            "agentic-traceability",
            validate_traceability(&pages),
            "leaf pages include source and line/byte range metadata".into(),
        ),
        scored_check(
            "agentic-link-graph",
            broken_local_links == 0 && unresolved_links == EXPECTED_UNRESOLVED_LINKS,
            format!(
                "broken local links: {broken_local_links}, unresolved: {unresolved_links}, expected: {EXPECTED_UNRESOLVED_LINKS}"
            ),
        ),
        scored_check(
            "agentic-no-full-read",
            validate_no_full_read(&out_a, &pages),
            "agent guide forbids full leaf scan and indexes fit page budget".into(),
        ),
        scored_check(
            "agentic-determinism",
            byte_identical_rerun && !input_hash_changed,
            format!(
                "byte-identical rerun: {byte_identical_rerun}, input hash changed: {input_hash_changed}"
            ),
        ),
    ];

    let score: f64 = checks.iter().map(|check| check.points).sum();
    let max_score: f64 = checks.iter().map(|check| check.max_points).sum();
    let passed = checks.iter().all(|check| check.passed);

    let report = GateReport {
        schema_version: SCHEMA_VERSION,
        mode: options.mode,
        passed,
        score,
        max_score,
        summary: GateSummary {
            fixtures: 0,
            input_bytes,
            generated_pages,
            max_page_chars,
            oversized_pages,
            broken_local_links,
            unresolved_links,
            expected_unresolved_links: EXPECTED_UNRESOLVED_LINKS,
            input_hash_changed: false,
            byte_identical_rerun: false,
            peak_rss_bytes: None,
            elapsed_ms: started.elapsed().as_millis(),
            max_buffered_bytes: 0,
        },
        query_simulations,
        checks,
    };
    let report = GateReport {
        summary: GateSummary {
            fixtures: fixtures.len(),
            input_bytes,
            generated_pages,
            max_page_chars,
            oversized_pages,
            broken_local_links,
            unresolved_links,
            expected_unresolved_links: EXPECTED_UNRESOLVED_LINKS,
            input_hash_changed,
            byte_identical_rerun,
            peak_rss_bytes: None,
            elapsed_ms: started.elapsed().as_millis(),
            max_buffered_bytes: 0,
        },
        ..report
    };

    if let Some(path) = &options.report_path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(path, serde_json::to_string_pretty(&report)?)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }

    if let Some(min_score) = options.min_score
        && report.score < min_score
    {
        bail!(
            "agentic search gate score {} is below required minimum {}",
            report.score,
            min_score
        );
    }

    Ok(report)
}

fn check_points(id: &str) -> f64 {
    match id {
        "agentic-entrypoint" => 8.0,
        "agentic-page-budget" => 12.0,
        "agentic-metadata" => 12.0,
        "agentic-catalog" => 8.0,
        "agentic-term-index" => 8.0,
        "agentic-navigation" => 8.0,
        "agentic-routing" => 6.0,
        "agentic-query-fixtures" => 10.0,
        "agentic-traceability" => 10.0,
        "agentic-link-graph" => 10.0,
        "agentic-no-full-read" => 4.0,
        "agentic-determinism" => 4.0,
        _ => 0.0,
    }
}

fn scored_check(id: &'static str, passed: bool, detail: String) -> GateCheck {
    let max_points = check_points(id);
    GateCheck {
        id,
        passed,
        points: if passed { max_points } else { 0.0 },
        max_points,
        detail,
    }
}

fn fixture_bytes(options: &GateOptions) -> usize {
    options
        .fixture_bytes_override
        .unwrap_or(match options.mode {
            GateMode::Normal => DEFAULT_NORMAL_FIXTURE_BYTES,
            GateMode::Heavy => DEFAULT_HEAVY_FIXTURE_BYTES,
        })
}

fn write_fixtures(dir: &Path, large_bytes: usize) -> Result<Vec<PathBuf>> {
    let specs = [
        (
            "agent-heading-lookup.md",
            "# Heading Lookup\n\n## Marker Heading 314\n\nThe marker heading 314 specification lives here.\n",
        ),
        (
            "agent-no-heading.md",
            "marker-no-heading-2048 appears in a note without headings.\n\nRepeated plain text supports exact phrase routing.\n",
        ),
        (
            "agent-links.md",
            "# Links\n\nTopic A points to [[agent-heading-lookup#Marker Heading 314]] and [Topic A label](../agent-heading-lookup/index.md).\n",
        ),
        (
            "agent-tags.md",
            "---\ntitle: Tagged Auth\ntags: [auth/session]\naliases: [session-auth]\n---\n# Tagged Auth\n\nAuth session material.\n",
        ),
        (
            "agent-unresolved.md",
            "# Unresolved\n\nThis page has [[definitely-missing-target]].\n",
        ),
        (
            "agent-ambiguous.md",
            "# Ambiguous\n\n## Design\n\nFirst design candidate.\n\n## Design\n\nSecond design candidate.\n",
        ),
        (
            "scenario/politics/norvas.md",
            "# ノルヴァス王国\n\n## 政治設定\n\nノルヴァス王国は北方の王政国家。\n",
        ),
        (
            "scenario/politics/protagonist-route.md",
            "# 政治ルート\n\n## 主人公関連\n\n政治ルートでは主人公が評議会と交渉する。\n",
        ),
        (
            "characters/heine-grunwald.md",
            "---\naliases: [ハイネ]\n---\n# ハイネ・グリュンヴァルト\n\n## 関係性\n\n政治ルートで主人公を支援する。\n",
        ),
        (
            "backbone/geography-map.md",
            "# 地理マップ\n\n## 交易路\n\n地理マップから主要な交易路を辿れる。\n",
        ),
        (
            "characters/relationships.md",
            "# キャラクター一覧\n\n## 関係性\n\n人物間の関係性を見るための入口。\n",
        ),
    ];
    let mut paths = Vec::new();
    for (name, body) in specs {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, body)?;
        paths.push(path);
    }
    let single_line = dir.join("agent-single-line.md");
    let mut body = String::from("# Single Line\n\n");
    body.push_str("single-line-marker-99999 ");
    while body.len() < large_bytes {
        body.push_str("single-line-padding ");
    }
    fs::write(&single_line, body)?;
    paths.push(single_line);
    Ok(paths)
}

fn simulate_agent_queries(root: &Path) -> Vec<QuerySimulation> {
    let specs = [
        (
            "ノルヴァス王国の設定はどこ？",
            vec![
                "index.md",
                "fragments/_index.md",
                "fragments/scenario/_index.md",
                "fragments/scenario/politics/_index.md",
                "fragments/scenario/politics/norvas/政治設定.md",
            ],
            "fragments/scenario/politics/norvas/政治設定.md",
        ),
        (
            "政治ルートの主人公関連を探す",
            vec![
                "index.md",
                "fragments/_index.md",
                "fragments/scenario/_index.md",
                "fragments/scenario/politics/_index.md",
                "fragments/scenario/politics/protagonist-route/主人公関連.md",
            ],
            "fragments/scenario/politics/protagonist-route/主人公関連.md",
        ),
        (
            "ハイネ・グリュンヴァルトが出るページ",
            vec![
                "index.md",
                "agent/terms/index.md",
                "fragments/characters/heine-grunwald/index.md",
            ],
            "fragments/characters/heine-grunwald/index.md",
        ),
        (
            "地理マップから交易路を辿る",
            vec![
                "index.md",
                "fragments/_index.md",
                "fragments/backbone/_index.md",
                "fragments/backbone/geography-map/交易路.md",
            ],
            "fragments/backbone/geography-map/交易路.md",
        ),
        (
            "キャラクター一覧から関係性を見る",
            vec![
                "index.md",
                "fragments/_index.md",
                "fragments/characters/_index.md",
                "fragments/characters/relationships/関係性.md",
            ],
            "fragments/characters/relationships/関係性.md",
        ),
    ];

    specs
        .into_iter()
        .map(|(query, route, expected)| {
            let route: Vec<PathBuf> = route.into_iter().map(PathBuf::from).collect();
            let expected_page = PathBuf::from(expected);
            let reached = route.iter().all(|path| root.join(path).exists())
                && root.join(&expected_page).exists();
            QuerySimulation {
                query,
                read_count: route.len(),
                route,
                expected_page,
                reached,
            }
        })
        .collect()
}

fn generate_wiki(fixture_dir: &Path, output: &Path) -> Result<()> {
    let files = scan(&ScanConfig {
        root: fixture_dir.to_path_buf(),
        extra_excluded: vec![output.to_path_buf()],
        recursive: true,
    });
    let classified = classify_scanned(fixture_dir, &files);
    let regular_files: Vec<_> = files
        .iter()
        .filter(|file| {
            classified.iter().any(|class| {
                class.relative_path == file.relative_path
                    && class.kind == InputKind::RegularMarkdown
            })
        })
        .cloned()
        .collect();
    let large_files: Vec<_> = classified
        .iter()
        .filter(|class| class.kind == InputKind::LargeMarkdown)
        .map(|class| class.relative_path.clone())
        .collect();

    let notes = ingest_notes(&regular_files, fixture_dir);
    let mut nodes = build_nodes(notes);
    let (unresolved, graph) = resolve_all(&nodes);
    let tag_index = build_tag_index(&nodes);
    compute_relations(&mut nodes, &graph, &tag_index);
    write_wiki(
        output,
        &WikiOutput {
            project_title: "agentic-search-fixture",
            nodes: &nodes,
            unresolved: &unresolved,
            graph: &graph,
        },
    )?;
    write_large_markdown_pages(output, fixture_dir, &large_files)?;
    finalize_agentic_output(output)?;
    Ok(())
}

fn validate_entrypoint(root: &Path, pages: &[(PathBuf, String)]) -> bool {
    let Ok(index) = fs::read_to_string(root.join("index.md")) else {
        return false;
    };
    [
        "agent/index.md",
        "fragments/_index.md",
        "headings/index.md",
        "links/index.md",
        "tags/index.md",
        "_unresolved.md",
    ]
    .iter()
    .all(|target| index.contains(target) && root.join(target).exists())
        && pages
            .iter()
            .any(|(path, _)| path == Path::new("agent/index.md"))
}

fn validate_metadata(pages: &[(PathBuf, String)]) -> bool {
    !pages.is_empty()
        && pages.iter().all(|(_, body)| {
            body.starts_with("---\nmd_wiki:\n")
                && body.contains("  schema_version: 1\n")
                && body.contains("  page_kind: ")
                && body.contains("  output_path: ")
                && body.contains("  char_count: ")
        })
}

fn validate_catalog(root: &Path, pages: &[(PathBuf, String)]) -> Result<bool> {
    let catalog = fs::read_to_string(root.join("agent/pages/index.md"))?;
    let page_catalog = if catalog.contains("| path | kind |") {
        catalog
    } else {
        let mut combined = catalog;
        for (rel, _) in markdown_files(&root.join("agent/pages"))?
            .into_iter()
            .filter(|(path, _)| path.file_name().and_then(|s| s.to_str()) != Some("index.md"))
        {
            combined.push_str(&fs::read_to_string(root.join("agent/pages").join(rel))?);
        }
        combined
    };
    let mut seen = std::collections::BTreeSet::new();
    for (path, _) in pages {
        if path.starts_with("agent/pages/page-") {
            continue;
        }
        if !page_catalog.contains(&markdown_path(path)) {
            return Ok(false);
        }
        if !seen.insert(path.clone()) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn validate_term_index(root: &Path) -> bool {
    let Ok(terms) = read_tree_text(&root.join("agent/terms")) else {
        return false;
    };
    [
        "marker heading 314",
        "auth/session",
        "agent-heading-lookup",
        "topic a label",
        "design",
    ]
    .iter()
    .all(|term| terms.to_lowercase().contains(term))
}

fn validate_navigation(root: &Path, pages: &[(PathBuf, String)]) -> Result<bool> {
    for (rel, body) in pages {
        let from_dir = rel.parent().unwrap_or(Path::new(""));
        for line in body.lines().filter(|line| {
            line.starts_with("> Parent: ")
                || line.trim_start().starts_with("- [")
                || line.contains("](")
        }) {
            for target in markdown_link_targets(line)? {
                if is_external_link(&target) {
                    continue;
                }
                let target_path = target.split('#').next().unwrap_or(&target);
                if target_path.is_empty() {
                    continue;
                }
                let normalized = normalize_relative(from_dir, Path::new(target_path));
                if !root.join(normalized).exists() {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn validate_routing(root: &Path) -> bool {
    let Ok(agent) = fs::read_to_string(root.join("agent/index.md")) else {
        return false;
    };
    [
        "agent/terms/",
        "headings/",
        "tags/",
        "links/",
        "_unresolved.md",
        "agent/pages/",
    ]
    .iter()
    .all(|needle| agent.contains(needle))
}

fn validate_query_fixtures(root: &Path) -> bool {
    let Ok(all) = read_tree_text(root) else {
        return false;
    };
    [
        "marker heading 314",
        "marker-no-heading-2048",
        "auth/session",
        "definitely-missing-target",
        "single-line-marker-99999",
        "Design",
        "ノルヴァス王国",
        "政治ルート",
        "ハイネ・グリュンヴァルト",
        "交易路",
    ]
    .iter()
    .all(|needle| all.contains(needle))
}

fn validate_traceability(pages: &[(PathBuf, String)]) -> bool {
    let leaf_pages: Vec<_> = pages
        .iter()
        .filter(|(_, body)| body.contains("  page_kind: leaf\n"))
        .collect();
    !leaf_pages.is_empty()
        && leaf_pages
            .iter()
            .all(|(_, body)| body.contains("  source: ") && body.contains("  line_ranges:"))
        && leaf_pages
            .iter()
            .any(|(_, body)| body.contains("  byte_ranges:"))
}

fn validate_no_full_read(root: &Path, pages: &[(PathBuf, String)]) -> bool {
    let Ok(agent) = fs::read_to_string(root.join("agent/index.md")) else {
        return false;
    };
    agent.contains("Do not read every leaf page")
        && pages
            .iter()
            .filter(|(path, _)| {
                path.starts_with("agent")
                    || path.starts_with("headings")
                    || path.starts_with("links")
                    || path.starts_with("tags")
                    || path.file_name().and_then(|s| s.to_str()) == Some("_index.md")
            })
            .all(|(_, body)| body.chars().count() <= 40_000)
}

fn snapshot_dir(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut out = BTreeMap::new();
    if !root.exists() {
        return Ok(out);
    }
    for path in all_files(root)? {
        let rel = path.strip_prefix(root)?.to_path_buf();
        out.insert(rel, fs::read(path)?);
    }
    Ok(out)
}

fn markdown_files(root: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    for path in all_files(root)? {
        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            let rel = path.strip_prefix(root)?.to_path_buf();
            out.push((rel, fs::read_to_string(path)?));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
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
            if entry.file_type()?.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn count_broken_local_links(root: &Path, pages: &[(PathBuf, String)]) -> Result<usize> {
    let mut broken = 0;
    for (rel, body) in pages {
        let from_dir = rel.parent().unwrap_or(Path::new(""));
        for target in markdown_link_targets(body)? {
            if is_external_link(&target) {
                continue;
            }
            let path_part = target.split('#').next().unwrap_or(&target);
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

fn count_unresolved_links(pages: &[(PathBuf, String)]) -> usize {
    pages
        .iter()
        .map(|(_, body)| body.matches("(未解決)").count())
        .sum()
}

fn markdown_link_targets(body: &str) -> Result<Vec<String>> {
    let re = Regex::new(r"\[[^\]\n]+\]\(([^)\s]+)\)")?;
    Ok(re
        .captures_iter(body)
        .map(|cap| cap[1].to_string())
        .collect())
}

fn is_external_link(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://") || target.starts_with("mailto:")
}

fn normalize_relative(base: &Path, rel: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in base.join(rel).components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            Component::Normal(part) => out.push(part),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    out
}

fn read_tree_text(root: &Path) -> Result<String> {
    let mut text = String::new();
    for path in all_files(root)? {
        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            text.push_str(&fs::read_to_string(path)?);
            text.push('\n');
        }
    }
    Ok(text)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn normal_report_contains_all_check_ids() {
        let temp = tempfile::tempdir().unwrap();
        let report = run_gate(&GateOptions {
            mode: GateMode::Normal,
            work_dir: temp.path().join("work"),
            report_path: None,
            min_score: Some(0.0),
            require_resource_budget: false,
            fixture_bytes_override: None,
        })
        .unwrap();

        assert_eq!(report.schema_version, 1);
        assert_eq!(report.mode, GateMode::Normal);
        assert_eq!(report.checks.len(), CHECK_IDS.len());
        for id in CHECK_IDS {
            assert!(
                report.checks.iter().any(|check| check.id == *id),
                "missing check id: {id}"
            );
        }
    }

    #[test]
    fn fixtures_are_deterministic() {
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        write_fixtures(&a, 128 * 1024).unwrap();
        write_fixtures(&b, 128 * 1024).unwrap();
        assert_eq!(snapshot_dir(&a).unwrap(), snapshot_dir(&b).unwrap());
    }

    #[test]
    fn fixtures_cover_required_query_classes() {
        let temp = tempfile::tempdir().unwrap();
        let paths = write_fixtures(temp.path(), 128 * 1024).unwrap();
        assert_eq!(paths.len(), 12);
        let all = read_tree_text(temp.path()).unwrap();
        for marker in [
            "marker heading 314",
            "marker-no-heading-2048",
            "Topic A",
            "auth/session",
            "definitely-missing-target",
            "## Design",
            "single-line-marker-99999",
            "ノルヴァス王国",
            "政治ルート",
            "ハイネ・グリュンヴァルト",
            "交易路",
        ] {
            assert!(all.contains(marker), "missing fixture marker: {marker}");
        }
    }

    #[test]
    fn catalog_validation_matches_markdown_paths() {
        let temp = tempfile::tempdir().unwrap();
        let catalog_dir = temp.path().join("agent/pages");
        fs::create_dir_all(&catalog_dir).unwrap();
        fs::write(
            catalog_dir.join("index.md"),
            "| path | kind |\n| --- | --- |\n| agent/index.md | guide |\n",
        )
        .unwrap();

        let pages = vec![(PathBuf::from("agent\\index.md"), String::new())];

        assert!(validate_catalog(temp.path(), &pages).unwrap());
    }

    #[test]
    fn simulated_agent_routes_are_reported() {
        let temp = tempfile::tempdir().unwrap();
        let report = run_gate(&GateOptions {
            mode: GateMode::Normal,
            work_dir: temp.path().join("work"),
            report_path: None,
            min_score: Some(0.0),
            require_resource_budget: false,
            fixture_bytes_override: Some(128 * 1024),
        })
        .unwrap();

        assert!(report.query_simulations.len() >= 5);
        assert!(
            report
                .query_simulations
                .iter()
                .any(|sim| sim.query.contains("ノルヴァス王国"))
        );
        for sim in &report.query_simulations {
            assert!(sim.reached, "query did not reach target: {}", sim.query);
            assert!(
                sim.read_count <= 5,
                "route should avoid full leaf scans: {sim:?}"
            );
        }
    }
}
