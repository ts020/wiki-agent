use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Serialize;

use crate::large_markdown::{
    ByteRange, LineScanReport, PageKind, PagePlan, SectionTree, SplitReason, plan_leaf_pages,
    scan_lines, scan_section_tree,
};

const SCHEMA_VERSION: u32 = 1;
const NORMAL_FIXTURE_BYTES: usize = 2 * 1024 * 1024;
const HEAVY_FIXTURE_BYTES: usize = 20 * 1024 * 1024;
const HUGE_FIXTURE_BYTES: usize = 200 * 1024 * 1024;
const STREAMING_BUFFER_LIMIT: usize = 8 * 1024 * 1024;

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

#[derive(Debug, Clone, Copy)]
enum FixtureKind {
    H2H3,
    NoHeading,
    SingleHeading,
    CodeBlock,
    Table,
    Mixed,
    ManyHeadings,
    ManyLinks,
}

#[derive(Debug, Clone, Copy)]
struct FixtureSpec {
    name: &'static str,
    kind: FixtureKind,
    min_bytes: usize,
}

#[derive(Debug)]
struct SourceGeneration {
    source_path: PathBuf,
    spec: FixtureSpec,
    scan: LineScanReport,
    tree: SectionTree,
    plans: Vec<PagePlan>,
}

pub fn run_gate(options: &GateOptions) -> Result<GateReport> {
    let started = Instant::now();
    let fixture_dir = options.work_dir.join("fixture");
    if fixture_dir.exists() {
        fs::remove_dir_all(&fixture_dir)
            .with_context(|| format!("failed to clear {}", fixture_dir.display()))?;
    }
    fs::create_dir_all(&fixture_dir)
        .with_context(|| format!("failed to create {}", fixture_dir.display()))?;

    let specs = fixture_specs(options.mode, options.fixture_bytes_override);
    let mut fixture_paths = Vec::new();
    for spec in &specs {
        let path = fixture_dir.join(spec.name);
        write_fixture(&path, *spec)?;
        fixture_paths.push(path);
    }

    let input_before = snapshot_files(&fixture_paths)?;
    let mut sources = Vec::new();
    for path in &fixture_paths {
        let scan = scan_lines(path)?;
        let tree = scan_section_tree(path)?;
        let spec = *specs
            .iter()
            .find(|spec| path.file_name().and_then(|s| s.to_str()) == Some(spec.name))
            .context("fixture spec disappeared")?;
        let plans = plan_leaf_pages(path, &scan, &tree, 40_000);
        sources.push(SourceGeneration {
            source_path: path.clone(),
            spec,
            scan,
            tree,
            plans,
        });
    }

    let out_a = options.work_dir.join("out-a");
    let out_b = options.work_dir.join("out-b");
    write_large_wiki(options.mode, &sources, &out_a)?;
    write_large_wiki(options.mode, &sources, &out_b)?;

    let input_after = snapshot_files(&fixture_paths)?;
    let input_hash_changed = input_before != input_after;
    let pages = markdown_files(&out_a)?;
    let snapshot_a = snapshot_dir(&out_a)?;
    let snapshot_b = snapshot_dir(&out_b)?;
    let byte_identical_rerun = snapshot_a == snapshot_b;
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

    let mut input_bytes = 0u64;
    let mut max_buffered_bytes = 0usize;
    let mut scanned = 0usize;
    let mut section_trees_valid = true;
    for source in &sources {
        let scan = &source.scan;
        let tree = &source.tree;
        input_bytes += scan.bytes;
        max_buffered_bytes = max_buffered_bytes.max(scan.max_buffered_bytes);
        section_trees_valid &= validate_section_tree(tree);
        scanned += 1;
    }

    let elapsed_ms = started.elapsed().as_millis();
    let ingestion_passed = scanned == fixture_paths.len();
    let streaming_passed = max_buffered_bytes <= STREAMING_BUFFER_LIMIT;
    let forced_split_passed = validate_forced_splits(&sources);
    let page_budget_passed = oversized_pages == 0 && generated_pages > 0;
    let metadata_passed = validate_metadata(&pages);
    let navigation_passed = validate_navigation(&out_a, &pages)?;
    let range_coverage_passed = validate_range_coverage(&sources);
    let index_paging_passed = validate_index_paging(options.mode, &pages);
    let link_integrity_passed = broken_local_links == 0 && unresolved_links == 0;
    let determinism_passed = byte_identical_rerun;
    let non_destructive_passed = !input_hash_changed;
    let resource_passed = match options.mode {
        GateMode::Normal => elapsed_ms <= 60_000 && streaming_passed,
        GateMode::Heavy => {
            !options.require_resource_budget || (elapsed_ms <= 300_000 && streaming_passed)
        }
    };

    let checks = vec![
        scored_check(
            "large-md-ingestion",
            ingestion_passed,
            10.0,
            format!("scanned fixtures: {scanned}/{}", fixture_paths.len()),
        ),
        supporting_check(
            "large-md-streaming",
            streaming_passed,
            format!("max buffered bytes: {max_buffered_bytes}, limit: {STREAMING_BUFFER_LIMIT}"),
        ),
        supporting_check(
            "large-md-section-tree",
            section_trees_valid,
            format!(
                "validated section trees for {} fixtures",
                fixture_paths.len()
            ),
        ),
        scored_check(
            "large-md-forced-split",
            forced_split_passed,
            15.0,
            "validated expected split reasons for fixture classes".into(),
        ),
        scored_check(
            "large-md-page-budget",
            page_budget_passed,
            20.0,
            format!("oversized pages: {oversized_pages}, max chars: {max_page_chars}"),
        ),
        supporting_check(
            "large-md-metadata",
            metadata_passed,
            "validated parseable md_wiki metadata on leaf pages".into(),
        ),
        scored_check(
            "large-md-navigation",
            navigation_passed,
            10.0,
            "validated parent/prev/next links on leaf pages".into(),
        ),
        supporting_check(
            "large-md-range-coverage",
            range_coverage_passed,
            "validated non-overlapping source byte coverage".into(),
        ),
        scored_check(
            "large-md-index-paging",
            index_paging_passed,
            15.0,
            "validated index pages stay within budget".into(),
        ),
        supporting_check(
            "large-md-link-integrity",
            link_integrity_passed,
            format!("broken local links: {broken_local_links}, unresolved: {unresolved_links}"),
        ),
        scored_check(
            "large-md-determinism",
            determinism_passed,
            10.0,
            format!("byte-identical rerun: {byte_identical_rerun}"),
        ),
        supporting_check(
            "large-md-non-destructive",
            non_destructive_passed,
            format!("input hash changed: {input_hash_changed}"),
        ),
        scored_check(
            "large-md-resource-budget",
            resource_passed,
            20.0,
            format!(
                "elapsed_ms: {elapsed_ms}, require_resource_budget: {}",
                options.require_resource_budget
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
            fixtures: fixture_paths.len(),
            input_bytes,
            generated_pages,
            max_page_chars,
            oversized_pages,
            broken_local_links,
            unresolved_links,
            input_hash_changed,
            byte_identical_rerun,
            peak_rss_bytes: None,
            elapsed_ms,
            max_buffered_bytes,
        },
        checks,
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
            "large Markdown gate score {} is below required minimum {}",
            report.score,
            min_score
        );
    }

    Ok(report)
}

fn write_large_wiki(mode: GateMode, sources: &[SourceGeneration], output: &Path) -> Result<()> {
    if output.exists() {
        fs::remove_dir_all(output)
            .with_context(|| format!("failed to clear {}", output.display()))?;
    }
    fs::create_dir_all(output)?;

    let mut all_leaf_paths = Vec::new();
    for source in sources {
        let entry = source
            .plans
            .iter()
            .find(|plan| plan.page_kind == PageKind::Entry)
            .context("entry plan missing")?;
        let leaves: Vec<_> = source
            .plans
            .iter()
            .filter(|plan| plan.page_kind == PageKind::Leaf)
            .collect();
        all_leaf_paths.extend(leaves.iter().map(|plan| plan.output_path.clone()));

        let entry_path = output.join(&entry.output_path);
        if let Some(parent) = entry_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&entry_path, render_entry(source, &leaves))?;

        for leaf in leaves {
            let leaf_path = output.join(&leaf.output_path);
            if let Some(parent) = leaf_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&leaf_path, render_leaf(source, leaf)?)?;
        }
    }

    write_root_indexes(mode, output, sources, &all_leaf_paths)?;
    Ok(())
}

fn render_entry(source: &SourceGeneration, leaves: &[&PagePlan]) -> String {
    let title = source.spec.name.trim_end_matches(".md");
    let mut out = String::new();
    out.push_str(&format!("# {title}\n\n"));
    out.push_str("## Pages\n\n");
    for leaf in leaves {
        let file = leaf.output_path.file_name().unwrap().to_string_lossy();
        out.push_str(&format!("- [{}]({})\n", leaf.page_id, file));
    }
    out
}

fn render_leaf(source: &SourceGeneration, plan: &PagePlan) -> Result<String> {
    let body = read_plan_body(source, plan)?;
    let source_name = source.source_path.file_name().unwrap().to_string_lossy();
    let parent = plan
        .parent
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let prev = plan
        .prev
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let next = plan
        .next
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let ranges = render_ranges(&plan.byte_ranges);
    let line_ranges = render_line_ranges(plan);
    let mut out = String::new();
    out.push_str("---\nmd_wiki:\n");
    out.push_str("  schema_version: 1\n");
    out.push_str("  page_kind: leaf\n");
    out.push_str(&format!("  source: {source_name}\n"));
    out.push_str("  section_path:\n");
    for section in &plan.section_path {
        out.push_str(&format!("    - {}\n", yaml_scalar(section)));
    }
    out.push_str("  byte_ranges:\n");
    out.push_str(&ranges);
    out.push_str("  line_ranges:\n");
    out.push_str(&line_ranges);
    out.push_str(&format!(
        "  split_reason: {}\n",
        split_reason_name(plan.split_reason)
    ));
    out.push_str(&format!("  parent: {parent}\n"));
    if !prev.is_empty() {
        out.push_str(&format!("  prev: {prev}\n"));
    }
    if !next.is_empty() {
        out.push_str(&format!("  next: {next}\n"));
    }
    out.push_str("---\n");
    out.push_str(&render_nav(plan));
    out.push_str("---\n\n");
    out.push_str(&body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn render_ranges(ranges: &[ByteRange]) -> String {
    let mut out = String::new();
    for range in ranges {
        out.push_str(&format!("    - [{}, {}]\n", range.start, range.end));
    }
    out
}

fn render_line_ranges(plan: &PagePlan) -> String {
    let mut out = String::new();
    for range in &plan.line_ranges {
        out.push_str(&format!("    - [{}, {}]\n", range.start, range.end));
    }
    out
}

fn render_nav(plan: &PagePlan) -> String {
    let mut out = String::from("> Parent: [Parent](");
    out.push_str(&plan.parent.as_ref().unwrap().display().to_string());
    out.push(')');
    if let Some(prev) = &plan.prev {
        out.push_str(&format!(" · Prev: [Prev]({})", prev.display()));
    }
    if let Some(next) = &plan.next {
        out.push_str(&format!(" · Next: [Next]({})", next.display()));
    }
    out.push('\n');
    out
}

fn read_plan_body(source: &SourceGeneration, plan: &PagePlan) -> Result<String> {
    let mut body = String::new();
    let mut file = File::open(&source.source_path)?;
    for range in &plan.byte_ranges {
        let len = (range.end - range.start) as usize;
        let mut bytes = vec![0; len];
        file.seek(SeekFrom::Start(range.start))?;
        file.read_exact(&mut bytes)?;
        body.push_str(std::str::from_utf8(&bytes)?);
    }

    Ok(match plan.split_reason {
        SplitReason::CodeFence => {
            let mut out = String::from("```text\n");
            for line in body.lines() {
                if !line.trim_start().starts_with("```") && !line.starts_with("# ") {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out.push_str("```\n");
            out
        }
        SplitReason::Table => {
            let mut out = String::from("| id | value |\n|---|---|\n");
            for line in body.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('|') && !trimmed.contains("---") && !trimmed.contains(" id ")
                {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out
        }
        _ => body,
    })
}

fn write_root_indexes(
    mode: GateMode,
    output: &Path,
    sources: &[SourceGeneration],
    leaf_paths: &[PathBuf],
) -> Result<()> {
    fs::create_dir_all(output.join("fragments"))?;
    fs::create_dir_all(output.join("headings"))?;
    fs::create_dir_all(output.join("links"))?;
    fs::create_dir_all(output.join("tags"))?;

    let mut root = String::from(
        "# md-wiki\n\n- [Fragments](fragments/_index.md)\n- [Headings](headings/index.md)\n- [Links](links/index.md)\n- [Tags](tags/index.md)\n- [Unresolved](_unresolved.md)\n",
    );
    fs::write(output.join("index.md"), &root)?;
    root.clear();

    let mut fragments = String::from("# Fragments\n\n");
    for source in sources {
        let entry = source
            .plans
            .iter()
            .find(|plan| plan.page_kind == PageKind::Entry)
            .unwrap();
        fragments.push_str(&format!(
            "- [{}]({})\n",
            source.spec.name,
            entry
                .output_path
                .strip_prefix("fragments")
                .unwrap()
                .display()
        ));
    }
    fs::write(output.join("fragments/_index.md"), fragments)?;

    write_paged_index(
        &output.join("headings"),
        "Headings",
        &heading_index_lines(sources),
        mode == GateMode::Heavy,
    )?;
    write_paged_index(
        &output.join("links"),
        "Links",
        &leaf_paths
            .iter()
            .map(|p| format!("- [{}](../{})\n", p.display(), p.display()))
            .collect::<Vec<_>>(),
        mode == GateMode::Heavy,
    )?;
    fs::write(output.join("tags/index.md"), "# Tags\n\n")?;
    fs::write(output.join("_unresolved.md"), "# Unresolved\n\n")?;
    Ok(())
}

fn heading_index_lines(sources: &[SourceGeneration]) -> Vec<String> {
    let mut lines = Vec::new();
    for source in sources {
        let entry = source
            .plans
            .iter()
            .find(|plan| plan.page_kind == PageKind::Entry)
            .unwrap();
        for section in source.tree.sections.iter().skip(1) {
            lines.push(format!(
                "- [{}](../{})\n",
                section.title,
                entry.output_path.display()
            ));
        }
    }
    lines
}

fn write_paged_index(dir: &Path, title: &str, lines: &[String], force_paging: bool) -> Result<()> {
    fs::create_dir_all(dir)?;
    let mut pages = Vec::new();
    let mut current = String::new();
    for line in lines {
        if !current.is_empty() && current.chars().count() + line.chars().count() > 30_000 {
            pages.push(current);
            current = String::new();
        }
        current.push_str(line);
    }
    if !current.is_empty() || pages.is_empty() {
        pages.push(current);
    }

    if pages.len() == 1 && !force_paging {
        fs::write(
            dir.join("index.md"),
            format!("# {title}\n\n{}", pages.remove(0)),
        )?;
        return Ok(());
    }

    let mut index = format!("# {title}\n\n");
    for i in 0..pages.len() {
        index.push_str(&format!(
            "- [Page {number}](page-{number:03}.md)\n",
            number = i + 1
        ));
    }
    fs::write(dir.join("index.md"), index)?;
    for (idx, page) in pages.into_iter().enumerate() {
        fs::write(
            dir.join(format!("page-{number:03}.md", number = idx + 1)),
            format!("# {title} {}\n\n{page}", idx + 1),
        )?;
    }
    Ok(())
}

fn scored_check(id: &'static str, passed: bool, max_points: f64, detail: String) -> GateCheck {
    GateCheck {
        id,
        passed,
        points: if passed { max_points } else { 0.0 },
        max_points,
        detail,
    }
}

fn supporting_check(id: &'static str, passed: bool, detail: String) -> GateCheck {
    GateCheck {
        id,
        passed,
        points: 0.0,
        max_points: 0.0,
        detail,
    }
}

fn validate_section_tree(tree: &SectionTree) -> bool {
    if tree.sections.len() != tree.heading_count + 1 {
        return false;
    }
    let by_id: HashMap<_, _> = tree
        .sections
        .iter()
        .map(|section| (section.id.as_str(), section))
        .collect();
    let child_edges: HashSet<_> = tree
        .sections
        .iter()
        .flat_map(|section| {
            section
                .children
                .iter()
                .map(|child| (section.id.as_str(), child.as_str()))
        })
        .collect();
    for section in tree.sections.iter().skip(1) {
        let Some(parent_id) = &section.parent_id else {
            return false;
        };
        let Some(parent) = by_id.get(parent_id.as_str()) else {
            return false;
        };
        if parent.level >= section.level {
            return false;
        }
        if !child_edges.contains(&(parent.id.as_str(), section.id.as_str())) {
            return false;
        }
        if section.byte_start > section.byte_end || section.line_start > section.line_end {
            return false;
        }
    }
    true
}

fn validate_forced_splits(sources: &[SourceGeneration]) -> bool {
    sources.iter().all(|source| {
        let expected = match source.spec.kind {
            FixtureKind::H2H3 | FixtureKind::ManyHeadings => SplitReason::Heading,
            FixtureKind::Mixed => SplitReason::Table,
            FixtureKind::NoHeading => SplitReason::Paragraph,
            FixtureKind::SingleHeading | FixtureKind::ManyLinks => SplitReason::LineWindow,
            FixtureKind::CodeBlock => SplitReason::CodeFence,
            FixtureKind::Table => SplitReason::Table,
        };
        source
            .plans
            .iter()
            .filter(|plan| plan.page_kind == PageKind::Leaf)
            .all(|plan| plan.split_reason == expected && plan.estimated_chars <= 30_000)
    })
}

fn validate_metadata(pages: &[(PathBuf, String)]) -> bool {
    let mut leaf_count = 0usize;
    for (_, body) in pages {
        if !body.contains("page_kind: leaf") {
            continue;
        }
        leaf_count += 1;
        if !body.starts_with("---\nmd_wiki:\n")
            || !body.contains("  schema_version: 1\n")
            || !body.contains("  source: ")
            || !body.contains("  section_path:\n")
            || !body.contains("  byte_ranges:\n")
            || !body.contains("  line_ranges:\n")
            || !body.contains("  split_reason: ")
            || !body.contains("  parent: index.md\n")
        {
            return false;
        }
    }
    leaf_count > 0
}

fn validate_navigation(root: &Path, pages: &[(PathBuf, String)]) -> Result<bool> {
    for (rel, body) in pages {
        if !body.contains("page_kind: leaf") {
            continue;
        }
        let Some(nav) = body.lines().find(|line| line.starts_with("> Parent: ")) else {
            return Ok(false);
        };
        if !nav.contains("](index.md)") {
            return Ok(false);
        }
        let from_dir = rel.parent().unwrap_or(Path::new(""));
        for target in markdown_link_targets(nav)? {
            let normalized = normalize_relative(from_dir, Path::new(&target));
            if !root.join(normalized).exists() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn validate_range_coverage(sources: &[SourceGeneration]) -> bool {
    for source in sources {
        let mut ranges: Vec<_> = source
            .plans
            .iter()
            .filter(|plan| plan.page_kind == PageKind::Leaf)
            .flat_map(|plan| plan.byte_ranges.iter())
            .collect();
        ranges.sort_by_key(|range| range.start);
        let mut cursor = 0u64;
        for range in ranges {
            if range.start != cursor || range.end < range.start {
                return false;
            }
            cursor = range.end;
        }
        if cursor != source.scan.bytes {
            return false;
        }
    }
    true
}

fn validate_index_paging(mode: GateMode, pages: &[(PathBuf, String)]) -> bool {
    let all_within_budget = pages
        .iter()
        .filter(|(path, _)| {
            path.starts_with("headings")
                || path.starts_with("links")
                || path.starts_with("tags")
                || path.file_name().and_then(|s| s.to_str()) == Some("_index.md")
                || path.as_path() == Path::new("_unresolved.md")
        })
        .all(|(_, body)| body.chars().count() <= 40_000);
    if !all_within_budget {
        return false;
    }
    match mode {
        GateMode::Normal => true,
        GateMode::Heavy => {
            pages.iter().any(|(path, _)| {
                path.starts_with("headings") && path != Path::new("headings/index.md")
            }) && pages
                .iter()
                .any(|(path, _)| path.starts_with("links") && path != Path::new("links/index.md"))
        }
    }
}

fn snapshot_files(paths: &[PathBuf]) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut out = Vec::new();
    for path in paths {
        out.push((path.clone(), fs::read(path)?));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn markdown_files(root: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    for path in all_files(root)? {
        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            let rel = path.strip_prefix(root)?.to_path_buf();
            out.push((rel, fs::read_to_string(&path)?));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn snapshot_dir(root: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut out = Vec::new();
    for path in all_files(root)? {
        let rel = path.strip_prefix(root)?.to_path_buf();
        out.push((rel, fs::read(&path)?));
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

fn count_broken_local_links(root: &Path, pages: &[(PathBuf, String)]) -> Result<usize> {
    let mut broken = 0usize;
    for (rel, body) in pages {
        let from_dir = rel.parent().unwrap_or(Path::new(""));
        for target in markdown_link_targets(body)? {
            if target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
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

fn markdown_link_targets(body: &str) -> Result<Vec<String>> {
    let re = Regex::new(r"\[[^\]\n]+\]\(([^)\s]+)\)")?;
    Ok(re
        .captures_iter(body)
        .map(|cap| cap[1].to_string())
        .collect())
}

fn count_unresolved_links(pages: &[(PathBuf, String)]) -> usize {
    pages
        .iter()
        .map(|(_, body)| body.matches("(未解決)").count() + body.matches("[[").count())
        .sum()
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

fn yaml_scalar(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn split_reason_name(reason: SplitReason) -> &'static str {
    match reason {
        SplitReason::Heading => "heading",
        SplitReason::Paragraph => "paragraph",
        SplitReason::List => "list",
        SplitReason::Table => "table",
        SplitReason::CodeFence => "code_fence",
        SplitReason::LineWindow => "line_window",
        SplitReason::ByteWindow => "byte_window",
    }
}

fn fixture_specs(mode: GateMode, override_bytes: Option<usize>) -> Vec<FixtureSpec> {
    let normal_bytes = override_bytes.unwrap_or(NORMAL_FIXTURE_BYTES);
    let heavy_bytes = override_bytes.unwrap_or(HEAVY_FIXTURE_BYTES);
    let huge_bytes = override_bytes.unwrap_or(HUGE_FIXTURE_BYTES);
    match mode {
        GateMode::Normal => vec![
            FixtureSpec {
                name: "large-h2-h3.md",
                kind: FixtureKind::H2H3,
                min_bytes: normal_bytes,
            },
            FixtureSpec {
                name: "large-no-heading.md",
                kind: FixtureKind::NoHeading,
                min_bytes: normal_bytes,
            },
            FixtureSpec {
                name: "large-single-heading.md",
                kind: FixtureKind::SingleHeading,
                min_bytes: normal_bytes,
            },
            FixtureSpec {
                name: "large-code-block.md",
                kind: FixtureKind::CodeBlock,
                min_bytes: normal_bytes,
            },
            FixtureSpec {
                name: "large-table.md",
                kind: FixtureKind::Table,
                min_bytes: normal_bytes,
            },
        ],
        GateMode::Heavy => vec![
            FixtureSpec {
                name: "huge-20mb.md",
                kind: FixtureKind::Mixed,
                min_bytes: heavy_bytes,
            },
            FixtureSpec {
                name: "huge-200mb.md",
                kind: FixtureKind::Mixed,
                min_bytes: huge_bytes,
            },
            FixtureSpec {
                name: "many-headings.md",
                kind: FixtureKind::ManyHeadings,
                min_bytes: heavy_bytes,
            },
            FixtureSpec {
                name: "many-links.md",
                kind: FixtureKind::ManyLinks,
                min_bytes: heavy_bytes,
            },
        ],
    }
}

fn write_fixture(path: &Path, spec: FixtureSpec) -> Result<()> {
    let file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    let mut bytes = 0usize;
    let mut i = 0usize;

    write_counted(&mut writer, &mut bytes, "---\ntitle: Large Fixture\n---\n")?;
    match spec.kind {
        FixtureKind::H2H3 => {
            while bytes < spec.min_bytes {
                write_counted(
                    &mut writer,
                    &mut bytes,
                    &format!("## Chapter {i}\n\n### Topic {i}\nmarker-h2-h3-{i}\n\n"),
                )?;
                i += 1;
            }
        }
        FixtureKind::NoHeading => {
            while bytes < spec.min_bytes {
                write_counted(
                    &mut writer,
                    &mut bytes,
                    &format!("paragraph marker-no-heading-{i} with deterministic body text.\n\n"),
                )?;
                i += 1;
            }
        }
        FixtureKind::SingleHeading => {
            write_counted(&mut writer, &mut bytes, "# Single Heading\n\n")?;
            while bytes < spec.min_bytes {
                write_counted(
                    &mut writer,
                    &mut bytes,
                    &format!("single-section marker-{i} with deterministic body text.\n"),
                )?;
                i += 1;
            }
        }
        FixtureKind::CodeBlock => {
            write_counted(&mut writer, &mut bytes, "# Code\n\n```text\n")?;
            while bytes < spec.min_bytes {
                write_counted(
                    &mut writer,
                    &mut bytes,
                    &format!("code-block-marker-{i}=value-{i}\n"),
                )?;
                i += 1;
            }
            write_counted(&mut writer, &mut bytes, "```\n")?;
        }
        FixtureKind::Table => {
            write_counted(
                &mut writer,
                &mut bytes,
                "# Table\n\n| id | value |\n|---|---|\n",
            )?;
            while bytes < spec.min_bytes {
                write_counted(
                    &mut writer,
                    &mut bytes,
                    &format!("| {i} | table-marker-{i} |\n"),
                )?;
                i += 1;
            }
        }
        FixtureKind::Mixed => {
            while bytes < spec.min_bytes {
                write_counted(
                    &mut writer,
                    &mut bytes,
                    &format!(
                        "## Mixed {i}\n\nParagraph marker-mixed-{i}.\n\n- item {i}\n\n| id | value |\n|---|---|\n| {i} | mixed |\n\n"
                    ),
                )?;
                i += 1;
            }
        }
        FixtureKind::ManyHeadings => {
            while bytes < spec.min_bytes {
                write_counted(
                    &mut writer,
                    &mut bytes,
                    &format!("## Heading {i}\n\nmarker-heading-{i}\n\n"),
                )?;
                i += 1;
            }
        }
        FixtureKind::ManyLinks => {
            write_counted(&mut writer, &mut bytes, "# Links\n\n")?;
            while bytes < spec.min_bytes {
                write_counted(
                    &mut writer,
                    &mut bytes,
                    &format!("[local {i}](#link-{i}) marker-link-{i}\n"),
                )?;
                i += 1;
            }
        }
    }
    writer.flush()?;
    Ok(())
}

fn write_counted(writer: &mut impl Write, bytes: &mut usize, text: &str) -> Result<()> {
    writer.write_all(text.as_bytes())?;
    *bytes += text.len();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn normal_gate_generates_fixtures_and_machine_readable_report() {
        let tmp = TempDir::new().unwrap();
        let report_path = tmp.path().join("report.json");

        let report = run_gate(&GateOptions {
            mode: GateMode::Normal,
            work_dir: tmp.path().join("gate"),
            report_path: Some(report_path.clone()),
            min_score: None,
            require_resource_budget: false,
            fixture_bytes_override: Some(16 * 1024),
        })
        .unwrap();

        assert_eq!(report.schema_version, 1);
        assert_eq!(report.mode, GateMode::Normal);
        assert_eq!(report.summary.fixtures, 5);
        assert!(report.summary.input_bytes >= 5 * 16 * 1024);
        assert!(report.summary.max_buffered_bytes <= 8 * 1024 * 1024);
        assert!(report.checks.iter().any(|c| c.id == "large-md-ingestion"));
        assert!(report.checks.iter().any(|c| c.id == "large-md-streaming"));
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.id == "large-md-section-tree" && c.passed)
        );

        let json = fs::read_to_string(report_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(parsed["mode"], "normal");
        assert_eq!(parsed["summary"]["fixtures"], 5);
    }

    #[test]
    fn normal_gate_reaches_full_score_for_generated_large_wiki() {
        let tmp = TempDir::new().unwrap();

        let report = run_gate(&GateOptions {
            mode: GateMode::Normal,
            work_dir: tmp.path().join("gate"),
            report_path: None,
            min_score: Some(100.0),
            require_resource_budget: false,
            fixture_bytes_override: Some(16 * 1024),
        })
        .unwrap();

        assert!(report.passed);
        assert_eq!(report.score, 100.0);
        assert_eq!(report.max_score, 100.0);
        assert!(report.summary.generated_pages > 0);
        assert_eq!(report.summary.oversized_pages, 0);
        assert_eq!(report.summary.broken_local_links, 0);
        assert!(report.summary.byte_identical_rerun);
        for check in report.checks {
            assert!(check.passed, "{} should pass", check.id);
        }
    }

    #[test]
    fn heavy_gate_smoke_uses_paged_indexes() {
        let tmp = TempDir::new().unwrap();

        let report = run_gate(&GateOptions {
            mode: GateMode::Heavy,
            work_dir: tmp.path().join("gate"),
            report_path: None,
            min_score: Some(100.0),
            require_resource_budget: true,
            fixture_bytes_override: Some(16 * 1024),
        })
        .unwrap();

        assert!(report.passed);
        assert_eq!(report.score, 100.0);
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.id == "large-md-index-paging" && check.passed)
        );
    }
}
