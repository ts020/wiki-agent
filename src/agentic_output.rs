use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use regex::Regex;

use crate::large_markdown::{self, LineRange, PageKind, PagePlan};
use crate::metadata_renderer::{Metadata, render_frontmatter};
use crate::notes::headings;

pub const PAGE_CHAR_LIMIT: usize = 40_000;
const MAX_TERM_PAGE_LINKS: usize = 20;

#[derive(Debug, Clone)]
struct CatalogRow {
    path: PathBuf,
    kind: String,
    title: String,
    source: String,
    section: String,
    chars: usize,
    parent: String,
    prev: String,
    next: String,
}

pub fn finalize_agentic_output(root: &Path) -> Result<()> {
    page_large_indexes(root)?;
    ensure_metadata_for_existing_pages(root)?;
    write_agent_guide(root)?;
    write_term_index(root)?;
    write_page_catalog(root)?;
    Ok(())
}

fn page_large_indexes(root: &Path) -> Result<()> {
    page_index_if_needed(root, Path::new("headings/index.md"), "Headings")?;
    page_index_if_needed(root, Path::new("links/index.md"), "Links")?;
    Ok(())
}

fn page_index_if_needed(root: &Path, rel: &Path, title: &str) -> Result<()> {
    let abs = root.join(rel);
    if !abs.exists() {
        return Ok(());
    }
    let body = strip_frontmatter(&fs::read_to_string(&abs)?).to_string();
    if body.chars().count() <= PAGE_CHAR_LIMIT {
        return Ok(());
    }
    let Some(dir_rel) = rel.parent() else {
        return Ok(());
    };
    let dir_abs = root.join(dir_rel);
    for entry in fs::read_dir(&dir_abs)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|name| name.starts_with("page-") && name.ends_with(".md"))
        {
            fs::remove_file(path)?;
        }
    }

    let mut pages = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0usize;
    for line in body.lines() {
        let line_chars = line.chars().count() + 1;
        if !current.is_empty() && current_chars + line_chars > 30_000 {
            pages.push(current);
            current = String::new();
            current_chars = 0;
        }
        current.push_str(line);
        current.push('\n');
        current_chars += line_chars;
    }
    if !current.is_empty() {
        pages.push(current);
    }

    let mut manifest = format!("# {title}\n\n- Paged index pages: `{}`\n", pages.len());
    for (idx, page) in pages.iter().enumerate() {
        let number = idx + 1;
        let summary = page_item_summary(page);
        let _ = writeln!(
            manifest,
            "- [Page {number}](page-{number:03}.md) — {} headings, `{}` ... `{}`",
            summary.count, summary.first, summary.last
        );
    }
    fs::write(&abs, manifest)?;

    for (idx, page) in pages.into_iter().enumerate() {
        let number = idx + 1;
        let body = format!("# {title} {number}\n\n{page}");
        fs::write(dir_abs.join(format!("page-{number:03}.md")), body)?;
    }
    Ok(())
}

struct PageItemSummary {
    count: usize,
    first: String,
    last: String,
}

fn page_item_summary(body: &str) -> PageItemSummary {
    let labels = markdown_link_labels(body);
    if labels.is_empty() {
        let headings: Vec<_> = headings::extract(strip_frontmatter(body))
            .into_iter()
            .map(|heading| heading.text)
            .collect();
        return PageItemSummary {
            count: headings.len(),
            first: headings.first().cloned().unwrap_or_default(),
            last: headings.last().cloned().unwrap_or_default(),
        };
    }
    PageItemSummary {
        count: labels.len(),
        first: labels.first().cloned().unwrap_or_default(),
        last: labels.last().cloned().unwrap_or_default(),
    }
}

pub fn write_large_markdown_pages(
    root: &Path,
    input_root: &Path,
    large_files: &[PathBuf],
) -> Result<()> {
    for rel in large_files {
        let abs = input_root.join(rel);
        let scan = large_markdown::scan_lines(&abs)?;
        let tree = large_markdown::scan_section_tree(&abs)?;
        let plans = large_markdown::plan_leaf_pages(rel, &scan, &tree, PAGE_CHAR_LIMIT);
        let leaves: Vec<_> = plans
            .iter()
            .filter(|plan| plan.page_kind == PageKind::Leaf)
            .collect();
        let entry = plans
            .iter()
            .find(|plan| plan.page_kind == PageKind::Entry)
            .context("large Markdown entry plan missing")?;
        write_large_entry(root, entry, &leaves)?;
        for leaf in leaves {
            write_large_leaf(root, &abs, leaf)?;
        }
    }
    Ok(())
}

fn write_large_entry(root: &Path, entry: &PagePlan, leaves: &[&PagePlan]) -> Result<()> {
    let title = entry
        .section_path
        .first()
        .cloned()
        .unwrap_or_else(|| "Large Markdown".into());
    let mut body = String::new();
    let _ = writeln!(body, "# {title}\n");
    let _ = writeln!(body, "- Leaf pages: `{}`\n", leaves.len());
    let _ = writeln!(body, "## Fragments\n");
    for leaf in leaves {
        let name = leaf.output_path.file_name().unwrap().to_string_lossy();
        let _ = writeln!(body, "- [{}]({name})", leaf.page_id);
    }
    let body = with_plan_metadata(entry, title, body, Vec::new());
    write_file(root, &entry.output_path, &body)
}

fn write_large_leaf(root: &Path, source_path: &Path, plan: &PagePlan) -> Result<()> {
    let mut content = String::new();
    let mut file = fs::File::open(source_path)?;
    for range in &plan.byte_ranges {
        let len = (range.end - range.start) as usize;
        let mut bytes = vec![0; len];
        file.seek(SeekFrom::Start(range.start))?;
        file.read_exact(&mut bytes)?;
        content.push_str(std::str::from_utf8(&bytes)?);
    }

    let mut body = String::new();
    body.push_str(&nav_line(plan));
    body.push_str("---\n\n");
    body.push_str(&content);
    if !body.ends_with('\n') {
        body.push('\n');
    }
    let title = plan
        .section_path
        .last()
        .cloned()
        .unwrap_or_else(|| plan.page_id.clone());
    let body = with_plan_metadata(plan, title, body, Vec::new());
    write_file(root, &plan.output_path, &body)
}

fn with_plan_metadata(
    plan: &PagePlan,
    title: String,
    body: String,
    children: Vec<PathBuf>,
) -> String {
    let meta = Metadata {
        page_kind: page_kind_name(plan.page_kind).into(),
        output_path: plan.output_path.clone(),
        title,
        source: plan.source_path.clone(),
        section_path: plan.section_path.clone(),
        heading_level: None,
        split_reason: Some(plan.split_reason),
        char_count: body.chars().count(),
        byte_ranges: plan.byte_ranges.clone(),
        line_ranges: plan.line_ranges.clone(),
        parent: plan.parent.clone(),
        prev: plan.prev.clone(),
        next: plan.next.clone(),
        children,
        tags: Vec::new(),
        outgoing_links: Vec::new(),
        backlinks_count: 0,
    };
    format!("{}{}", render_frontmatter(&meta), body)
}

fn nav_line(plan: &PagePlan) -> String {
    let mut out = String::new();
    if let Some(parent) = &plan.parent {
        let _ = write!(out, "> Parent: [Parent]({})", parent.display());
    }
    if let Some(prev) = &plan.prev {
        let _ = write!(out, " · Prev: [Prev]({})", prev.display());
    }
    if let Some(next) = &plan.next {
        let _ = write!(out, " · Next: [Next]({})", next.display());
    }
    out.push('\n');
    out
}

fn page_kind_name(kind: PageKind) -> &'static str {
    match kind {
        PageKind::Entry => "entry",
        PageKind::Shell => "shell",
        PageKind::Leaf => "leaf",
        PageKind::PagedIndex => "paged_index",
    }
}

fn ensure_metadata_for_existing_pages(root: &Path) -> Result<()> {
    for rel in markdown_files(root)? {
        if rel.starts_with("agent") {
            continue;
        }
        let abs = root.join(&rel);
        let body = fs::read_to_string(&abs)?;
        if body.starts_with("---\nmd_wiki:\n") {
            continue;
        }
        let meta = infer_metadata(root, &rel, &body);
        fs::write(&abs, format!("{}{}", render_frontmatter(&meta), body))?;
    }
    Ok(())
}

fn infer_metadata(root: &Path, rel: &Path, body: &str) -> Metadata {
    let title = first_heading(body).unwrap_or_else(|| rel.display().to_string());
    let page_kind = infer_page_kind(rel);
    let (parent, prev, next) = nav_targets(body);
    let children = child_links(root, rel, body);
    let source = source_for(rel);
    let tags = tags_for(rel);
    let outgoing_links = if page_kind == "paged_index" {
        Vec::new()
    } else {
        markdown_link_targets(body)
            .into_iter()
            .map(|target| {
                normalize_relative(rel.parent().unwrap_or(Path::new("")), Path::new(&target))
            })
            .filter(|target| root.join(target).exists())
            .collect()
    };
    Metadata {
        page_kind,
        output_path: rel.to_path_buf(),
        title,
        source,
        section_path: section_path_for(rel),
        heading_level: heading_level(body),
        split_reason: None,
        char_count: body.chars().count(),
        byte_ranges: Vec::new(),
        line_ranges: line_range_for(body),
        parent,
        prev,
        next,
        children,
        tags,
        outgoing_links,
        backlinks_count: body.matches("## Backlinks").count(),
    }
}

fn infer_page_kind(rel: &Path) -> String {
    if rel == Path::new("index.md")
        || rel == Path::new("_unresolved.md")
        || rel.file_name().and_then(|s| s.to_str()) == Some("_index.md")
        || rel.starts_with("headings")
        || rel.starts_with("links")
        || rel.starts_with("tags")
    {
        return "paged_index".into();
    }
    if rel.starts_with("fragments") && rel.file_name().and_then(|s| s.to_str()) == Some("index.md")
    {
        return "entry".into();
    }
    "leaf".into()
}

fn source_for(rel: &Path) -> Option<PathBuf> {
    let stripped = rel.strip_prefix("fragments").ok()?;
    let mut parts: Vec<_> = stripped
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        return None;
    }
    parts.pop();
    if parts.is_empty() {
        return None;
    }
    Some(PathBuf::from(format!("{}.md", parts.join("/"))))
}

fn tags_for(rel: &Path) -> Vec<String> {
    if !rel.starts_with("tags") || rel == Path::new("tags/index.md") {
        return Vec::new();
    }
    rel.strip_prefix("tags")
        .ok()
        .and_then(|path| path.with_extension("").to_str().map(str::to_string))
        .into_iter()
        .collect()
}

fn section_path_for(rel: &Path) -> Vec<String> {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}

fn heading_level(body: &str) -> Option<u8> {
    headings::extract(strip_frontmatter(body))
        .first()
        .map(|heading| heading.level)
}

fn line_range_for(body: &str) -> Vec<LineRange> {
    let lines = body.lines().count() as u64;
    if lines == 0 {
        Vec::new()
    } else {
        vec![LineRange {
            start: 1,
            end: lines,
        }]
    }
}

fn first_heading(body: &str) -> Option<String> {
    headings::extract(strip_frontmatter(body))
        .first()
        .map(|heading| heading.text.clone())
}

fn strip_frontmatter(body: &str) -> &str {
    if !body.starts_with("---\n") {
        return body;
    }
    let Some(rest) = body.strip_prefix("---\n") else {
        return body;
    };
    let Some(end) = rest.find("\n---\n") else {
        return body;
    };
    &rest[end + 5..]
}

fn nav_targets(body: &str) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    let Some(line) = body.lines().find(|line| line.starts_with("> Parent: ")) else {
        return (None, None, None);
    };
    static RE: OnceLock<Regex> = OnceLock::new();
    let mut parent = None;
    let mut prev = None;
    let mut next = None;
    for cap in RE
        .get_or_init(|| Regex::new(r"(Parent|Prev|Next): \[[^\]\n]+\]\(([^)]+)\)").unwrap())
        .captures_iter(line)
    {
        match &cap[1] {
            "Parent" => parent = Some(PathBuf::from(&cap[2])),
            "Prev" => prev = Some(PathBuf::from(&cap[2])),
            "Next" => next = Some(PathBuf::from(&cap[2])),
            _ => {}
        }
    }
    (parent, prev, next)
}

fn child_links(root: &Path, rel: &Path, body: &str) -> Vec<PathBuf> {
    if !body.contains("## Fragments") {
        return Vec::new();
    }
    markdown_link_targets(body)
        .into_iter()
        .map(|target| normalize_relative(rel.parent().unwrap_or(Path::new("")), Path::new(&target)))
        .filter(|target| root.join(target).exists())
        .collect()
}

fn write_agent_guide(root: &Path) -> Result<()> {
    let children = vec![
        PathBuf::from("../fragments/_index.md"),
        PathBuf::from("../headings/index.md"),
        PathBuf::from("../links/index.md"),
        PathBuf::from("../tags/index.md"),
        PathBuf::from("../_unresolved.md"),
        PathBuf::from("pages/index.md"),
        PathBuf::from("terms/index.md"),
    ];
    let mut body = String::new();
    body.push_str("# Agent Guide\n\n");
    body.push_str("- Page budget: 40,000 characters hard limit per Markdown file\n");
    body.push_str("- Do not read every leaf page in sequence; route through indexes first\n");
    body.push_str("- Keep source, line range, and generated page path as evidence\n\n");
    body.push_str("## Indexes\n\n");
    for child in &children {
        let label = child.display();
        let _ = writeln!(body, "- [{label}]({label})");
    }
    body.push_str("\n## Query Routing\n\n");
    body.push_str("| Query type | Primary route | Secondary route |\n");
    body.push_str("|---|---|---|\n");
    body.push_str("| definition | `agent/terms/`, `headings/` | `fragments/_index.md` |\n");
    body.push_str("| specification | `headings/`, text search | Prev / Next leaf pages |\n");
    body.push_str("| relationship | `links/`, Backlinks | related leaf pages |\n");
    body.push_str("| tag | `tags/` | `agent/terms/` |\n");
    body.push_str("| unresolved | `_unresolved.md` | page catalog |\n");
    body.push_str("| huge range | `agent/pages/` source and line range | Prev / Next |\n");
    body.push_str("| ambiguous | `agent/terms/` and text search | catalog comparison |\n");
    body.push('\n');
    body.push_str("## Search Steps\n\n");
    body.push_str("1. Read `index.md`.\n");
    body.push_str("2. Read `agent/index.md`.\n");
    body.push_str("3. Choose headings, tags, links, terms, or catalog according to the query.\n");
    body.push_str("4. Read candidate entry or shell pages, then selected leaf pages.\n");
    body.push_str(
        "5. Expand only through Prev, Next, Children, Backlinks, or outgoing links as needed.\n",
    );

    let meta = Metadata {
        page_kind: "agent_guide".into(),
        output_path: PathBuf::from("agent/index.md"),
        title: "Agent Guide".into(),
        source: None,
        section_path: vec!["agent".into()],
        heading_level: None,
        split_reason: None,
        char_count: body.chars().count(),
        byte_ranges: Vec::new(),
        line_ranges: Vec::new(),
        parent: Some(PathBuf::from("../index.md")),
        prev: None,
        next: None,
        children,
        tags: Vec::new(),
        outgoing_links: Vec::new(),
        backlinks_count: 0,
    };
    write_file(
        root,
        Path::new("agent/index.md"),
        &format!("{}{}", render_frontmatter(&meta), body),
    )
}

fn write_page_catalog(root: &Path) -> Result<()> {
    let mut rows = catalog_rows(root)?;
    rows.retain(|row| !row.path.starts_with("agent/pages"));
    let by_source_rows = write_by_source_catalog(root, &rows)?;
    rows.push(CatalogRow {
        path: PathBuf::from("agent/pages/index.md"),
        kind: "page_catalog".into(),
        title: "Page Catalog".into(),
        source: String::new(),
        section: "agent/pages".into(),
        chars: 0,
        parent: "../index.md".into(),
        prev: String::new(),
        next: String::new(),
    });
    rows.extend(by_source_rows);
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    let pages = paginate_catalog_rows(&rows);
    let children = if pages.len() <= 1 {
        Vec::new()
    } else {
        (0..pages.len())
            .map(|idx| PathBuf::from(format!("page-{number:03}.md", number = idx + 1)))
            .collect()
    };
    let mut index_body = String::from("# Page Catalog\n\n");
    if children.is_empty() {
        index_body.push_str(&catalog_table(&rows));
    } else {
        for (idx, child) in children.iter().enumerate() {
            let label = child.display();
            let page_rows = &pages[idx];
            let first = page_rows
                .first()
                .map(|row| row.path.display().to_string())
                .unwrap_or_default();
            let last = page_rows
                .last()
                .map(|row| row.path.display().to_string())
                .unwrap_or_default();
            let _ = writeln!(
                index_body,
                "- [Page {}]({label}) — {} pages, `{first}` to `{last}`",
                idx + 1,
                page_rows.len()
            );
        }
    }
    let index_meta = Metadata {
        page_kind: "page_catalog".into(),
        output_path: PathBuf::from("agent/pages/index.md"),
        title: "Page Catalog".into(),
        source: None,
        section_path: vec!["agent".into(), "pages".into()],
        heading_level: None,
        split_reason: None,
        char_count: index_body.chars().count(),
        byte_ranges: Vec::new(),
        line_ranges: Vec::new(),
        parent: Some(PathBuf::from("../index.md")),
        prev: None,
        next: None,
        children: children.clone(),
        tags: Vec::new(),
        outgoing_links: Vec::new(),
        backlinks_count: 0,
    };
    write_file(
        root,
        Path::new("agent/pages/index.md"),
        &format!("{}{}", render_frontmatter(&index_meta), index_body),
    )?;
    if children.is_empty() {
        return Ok(());
    }
    for (idx, page_rows) in pages.iter().enumerate() {
        let name = format!("page-{number:03}.md", number = idx + 1);
        let mut body = format!("# Page Catalog {}\n\n", idx + 1);
        body.push_str(&catalog_table(page_rows));
        let meta = Metadata {
            page_kind: "page_catalog".into(),
            output_path: PathBuf::from("agent/pages").join(&name),
            title: format!("Page Catalog {}", idx + 1),
            source: None,
            section_path: vec!["agent".into(), "pages".into()],
            heading_level: None,
            split_reason: None,
            char_count: body.chars().count(),
            byte_ranges: Vec::new(),
            line_ranges: Vec::new(),
            parent: Some(PathBuf::from("index.md")),
            prev: (idx > 0).then(|| PathBuf::from(format!("page-{number:03}.md", number = idx))),
            next: (idx + 1 < pages.len())
                .then(|| PathBuf::from(format!("page-{number:03}.md", number = idx + 2))),
            children: Vec::new(),
            tags: Vec::new(),
            outgoing_links: Vec::new(),
            backlinks_count: 0,
        };
        write_file(
            root,
            &PathBuf::from("agent/pages").join(name),
            &format!("{}{}", render_frontmatter(&meta), body),
        )?;
    }
    Ok(())
}

fn write_by_source_catalog(root: &Path, rows: &[CatalogRow]) -> Result<Vec<CatalogRow>> {
    let mut by_source: BTreeMap<String, Vec<CatalogRow>> = BTreeMap::new();
    for row in rows {
        if !row.source.is_empty() {
            by_source
                .entry(row.source.clone())
                .or_default()
                .push(row.clone());
        }
    }

    let mut generated_rows = Vec::new();
    let mut used_names: BTreeSet<String> = BTreeSet::new();
    let mut source_pages = Vec::new();
    for (source, source_rows) in by_source {
        let name = unique_source_page_name(&source, &mut used_names);
        let rel = PathBuf::from("agent/pages/by-source").join(&name);
        let mut body = format!("# {source}\n\n");
        body.push_str("| generated page | kind | title | chars |\n|---|---|---|---:|\n");
        for row in &source_rows {
            let _ = writeln!(
                body,
                "| [{}](../../../{}) | `{}` | {} | {} |",
                escape_table(&row.path.display().to_string()),
                row.path.display(),
                escape_table(&row.kind),
                escape_table(&row.title),
                row.chars
            );
        }
        let meta = Metadata {
            page_kind: "page_catalog".into(),
            output_path: rel.clone(),
            title: source.clone(),
            source: Some(PathBuf::from(&source)),
            section_path: vec!["agent".into(), "pages".into(), "by-source".into()],
            heading_level: None,
            split_reason: None,
            char_count: body.chars().count(),
            byte_ranges: Vec::new(),
            line_ranges: Vec::new(),
            parent: Some(PathBuf::from("index.md")),
            prev: None,
            next: None,
            children: Vec::new(),
            tags: Vec::new(),
            outgoing_links: source_rows.iter().map(|row| row.path.clone()).collect(),
            backlinks_count: 0,
        };
        write_file(
            root,
            &rel,
            &format!("{}{}", render_frontmatter(&meta), body),
        )?;
        generated_rows.push(CatalogRow {
            path: rel.clone(),
            kind: "page_catalog".into(),
            title: source.clone(),
            source: source.clone(),
            section: "agent/pages/by-source".into(),
            chars: body.chars().count(),
            parent: "index.md".into(),
            prev: String::new(),
            next: String::new(),
        });
        source_pages.push((source, name, source_rows.len()));
    }

    let mut index_body = String::from("# Pages By Source\n\n");
    if source_pages.is_empty() {
        index_body.push_str("_(no source-backed pages)_\n");
    } else {
        for (source, name, count) in &source_pages {
            let _ = writeln!(index_body, "- [{source}]({name}) — {count} pages");
        }
    }
    let index_rel = PathBuf::from("agent/pages/by-source/index.md");
    let children = source_pages
        .iter()
        .map(|(_, name, _)| PathBuf::from(name))
        .collect();
    let meta = Metadata {
        page_kind: "page_catalog".into(),
        output_path: index_rel.clone(),
        title: "Pages By Source".into(),
        source: None,
        section_path: vec!["agent".into(), "pages".into(), "by-source".into()],
        heading_level: None,
        split_reason: None,
        char_count: index_body.chars().count(),
        byte_ranges: Vec::new(),
        line_ranges: Vec::new(),
        parent: Some(PathBuf::from("../index.md")),
        prev: None,
        next: None,
        children,
        tags: Vec::new(),
        outgoing_links: Vec::new(),
        backlinks_count: 0,
    };
    write_file(
        root,
        &index_rel,
        &format!("{}{}", render_frontmatter(&meta), index_body),
    )?;
    generated_rows.push(CatalogRow {
        path: index_rel,
        kind: "page_catalog".into(),
        title: "Pages By Source".into(),
        source: String::new(),
        section: "agent/pages/by-source".into(),
        chars: index_body.chars().count(),
        parent: "../index.md".into(),
        prev: String::new(),
        next: String::new(),
    });

    Ok(generated_rows)
}

fn unique_source_page_name(source: &str, used: &mut BTreeSet<String>) -> String {
    let base = source_page_stem(source);
    let mut name = format!("{base}.md");
    let mut idx = 1usize;
    while used.contains(&name) {
        name = format!("{base}-{idx}.md");
        idx += 1;
    }
    used.insert(name.clone());
    name
}

fn source_page_stem(source: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for c in source.trim_end_matches(".md").chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            out.push(c);
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }
    let stem = out.trim_matches('_');
    if stem.is_empty() {
        "source".into()
    } else {
        stem.into()
    }
}

fn catalog_rows(root: &Path) -> Result<Vec<CatalogRow>> {
    let mut rows = Vec::new();
    for rel in markdown_files(root)? {
        let body = fs::read_to_string(root.join(&rel))?;
        rows.push(CatalogRow {
            kind: metadata_value(&body, "page_kind").unwrap_or_else(|| infer_page_kind(&rel)),
            title: metadata_value(&body, "title").unwrap_or_else(|| {
                first_heading(&body).unwrap_or_else(|| rel.display().to_string())
            }),
            source: metadata_value(&body, "source").unwrap_or_default(),
            section: section_path_for(&rel).join(" > "),
            chars: body.chars().count(),
            parent: metadata_value(&body, "parent").unwrap_or_default(),
            prev: metadata_value(&body, "prev").unwrap_or_default(),
            next: metadata_value(&body, "next").unwrap_or_default(),
            path: rel,
        });
    }
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(rows)
}

fn paginate_catalog_rows(rows: &[CatalogRow]) -> Vec<Vec<CatalogRow>> {
    let mut pages = Vec::new();
    let mut current = Vec::new();
    let mut current_chars = catalog_table_header().chars().count();
    for row in rows {
        let line = catalog_row(row);
        if !current.is_empty() && current_chars + line.chars().count() > 30_000 {
            pages.push(current);
            current = Vec::new();
            current_chars = catalog_table_header().chars().count();
        }
        current_chars += line.chars().count();
        current.push(row.clone());
    }
    if !current.is_empty() {
        pages.push(current);
    }
    pages
}

fn catalog_table(rows: &[CatalogRow]) -> String {
    let mut out = catalog_table_header();
    for row in rows {
        out.push_str(&catalog_row(row));
    }
    out
}

fn catalog_table_header() -> String {
    "| path | kind | title | source | section | chars | parent | prev | next |\n|---|---|---|---|---|---:|---|---|---|\n".into()
}

fn catalog_row(row: &CatalogRow) -> String {
    format!(
        "| [{}](../../{}) | `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
        escape_table(&row.path.display().to_string()),
        row.path.display(),
        escape_table(&row.kind),
        escape_table(&row.title),
        escape_table(&row.source),
        escape_table(&row.section),
        row.chars,
        escape_table(&row.parent),
        escape_table(&row.prev),
        escape_table(&row.next)
    )
}

fn write_term_index(root: &Path) -> Result<()> {
    let mut terms: BTreeMap<(String, String), BTreeSet<PathBuf>> = BTreeMap::new();
    for rel in markdown_files(root)? {
        if rel.starts_with("agent/terms") {
            continue;
        }
        if let Some(tag) = tag_term_for_page(&rel) {
            add_term(&mut terms, tag, "tag", &rel);
            continue;
        }
        if suppress_generated_term_source(&rel) {
            continue;
        }
        let body = fs::read_to_string(root.join(&rel))?;
        for heading in headings::extract(strip_frontmatter(&body)) {
            add_term(&mut terms, heading.text, "heading", &rel);
        }
        if let Some(title) = metadata_value(&body, "title") {
            add_term(&mut terms, title, "title", &rel);
        }
        if let Some(source) = metadata_value(&body, "source")
            && let Some(stem) = Path::new(&source).file_stem().and_then(|s| s.to_str())
        {
            add_term(&mut terms, stem.to_string(), "file", &rel);
        }
        for label in markdown_link_labels(&body) {
            add_term(&mut terms, label, "link_label", &rel);
        }
    }
    let rows: Vec<_> = terms.into_iter().collect();
    let pages = paginate_term_rows(&rows);
    let children: Vec<_> = if pages.len() <= 1 {
        Vec::new()
    } else {
        (0..pages.len())
            .map(|idx| PathBuf::from(format!("page-{number:03}.md", number = idx + 1)))
            .collect()
    };
    let mut body = String::from("# Term Index\n\n");
    if children.is_empty() {
        body.push_str(&term_table(&rows));
    } else {
        for child in &children {
            let label = child.display();
            let _ = writeln!(body, "- [{label}]({label})");
        }
    }
    let meta = Metadata {
        page_kind: "term_index".into(),
        output_path: PathBuf::from("agent/terms/index.md"),
        title: "Term Index".into(),
        source: None,
        section_path: vec!["agent".into(), "terms".into()],
        heading_level: None,
        split_reason: None,
        char_count: body.chars().count(),
        byte_ranges: Vec::new(),
        line_ranges: Vec::new(),
        parent: Some(PathBuf::from("../index.md")),
        prev: None,
        next: None,
        children: children.clone(),
        tags: Vec::new(),
        outgoing_links: Vec::new(),
        backlinks_count: 0,
    };
    write_file(
        root,
        Path::new("agent/terms/index.md"),
        &format!("{}{}", render_frontmatter(&meta), body),
    )?;
    for (idx, page_rows) in pages.iter().enumerate() {
        if children.is_empty() {
            break;
        }
        let name = format!("page-{number:03}.md", number = idx + 1);
        let mut body = format!("# Term Index {}\n\n", idx + 1);
        body.push_str(&term_table(page_rows));
        let meta = Metadata {
            page_kind: "term_index".into(),
            output_path: PathBuf::from("agent/terms").join(&name),
            title: format!("Term Index {}", idx + 1),
            source: None,
            section_path: vec!["agent".into(), "terms".into()],
            heading_level: None,
            split_reason: None,
            char_count: body.chars().count(),
            byte_ranges: Vec::new(),
            line_ranges: Vec::new(),
            parent: Some(PathBuf::from("index.md")),
            prev: (idx > 0).then(|| PathBuf::from(format!("page-{number:03}.md", number = idx))),
            next: (idx + 1 < pages.len())
                .then(|| PathBuf::from(format!("page-{number:03}.md", number = idx + 2))),
            children: Vec::new(),
            tags: Vec::new(),
            outgoing_links: Vec::new(),
            backlinks_count: 0,
        };
        write_file(
            root,
            &PathBuf::from("agent/terms").join(name),
            &format!("{}{}", render_frontmatter(&meta), body),
        )?;
    }
    Ok(())
}

fn tag_term_for_page(rel: &Path) -> Option<String> {
    if !rel.starts_with("tags") || rel == Path::new("tags/index.md") {
        return None;
    }
    rel.strip_prefix("tags")
        .ok()
        .and_then(|path| path.with_extension("").to_str().map(str::to_string))
}

fn suppress_generated_term_source(rel: &Path) -> bool {
    rel == Path::new("index.md")
        || rel == Path::new("_unresolved.md")
        || rel.file_name().and_then(|s| s.to_str()) == Some("_index.md")
        || rel.starts_with("headings")
        || rel.starts_with("links")
        || rel.starts_with("agent")
}

type TermRow = ((String, String), BTreeSet<PathBuf>);

fn add_term(
    terms: &mut BTreeMap<(String, String), BTreeSet<PathBuf>>,
    term: String,
    kind: &str,
    page: &Path,
) {
    let term = plain_link_label(&term).trim().to_lowercase();
    if term.is_empty() || is_suppressed_term(&term) {
        return;
    }
    terms
        .entry((term, kind.into()))
        .or_default()
        .insert(page.to_path_buf());
}

fn is_suppressed_term(term: &str) -> bool {
    static PAGE_RE: OnceLock<Regex> = OnceLock::new();
    matches!(
        term,
        "index" | "_index" | "headings" | "links" | "tags" | "fragments" | "page catalog"
    ) || PAGE_RE
        .get_or_init(|| Regex::new(r"^page-\d+$").unwrap())
        .is_match(term)
}

fn plain_link_label(term: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]\n]+)\]\([^)]+\)").unwrap())
        .replace_all(term, "$1")
        .to_string()
}

fn paginate_term_rows(rows: &[TermRow]) -> Vec<Vec<TermRow>> {
    let mut pages = Vec::new();
    let mut current = Vec::new();
    let mut chars = term_table_header().chars().count();
    for row in rows {
        let line = term_row(row);
        if !current.is_empty() && chars + line.chars().count() > 30_000 {
            pages.push(current);
            current = Vec::new();
            chars = term_table_header().chars().count();
        }
        chars += line.chars().count();
        current.push(row.clone());
    }
    if !current.is_empty() {
        pages.push(current);
    }
    pages
}

fn term_table(rows: &[TermRow]) -> String {
    let mut out = term_table_header();
    for row in rows {
        out.push_str(&term_row(row));
    }
    out
}

fn term_table_header() -> String {
    "| term | kind | pages |\n|---|---|---|\n".into()
}

fn term_row(row: &TermRow) -> String {
    let ((term, kind), pages) = row;
    let remaining = pages.len().saturating_sub(MAX_TERM_PAGE_LINKS);
    let mut page_links = pages
        .iter()
        .take(MAX_TERM_PAGE_LINKS)
        .map(|page| format!("[{}](../../{})", page.display(), page.display()))
        .collect::<Vec<_>>()
        .join(", ");
    if remaining > 0 {
        if !page_links.is_empty() {
            page_links.push_str(", ");
        }
        let _ = write!(
            page_links,
            "... {remaining} more; see [Page Catalog](../pages/index.md)"
        );
    }
    format!(
        "| {} | `{}` | {} |\n",
        escape_table(term),
        escape_table(kind),
        page_links
    )
}

fn metadata_value(body: &str, key: &str) -> Option<String> {
    let prefix = format!("  {key}:");
    body.lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_matches('"').to_string())
}

fn markdown_files(root: &Path) -> Result<Vec<PathBuf>> {
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
            } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                out.push(path.strip_prefix(root)?.to_path_buf());
            }
        }
    }
    out.sort();
    Ok(out)
}

fn markdown_link_targets(body: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[[^\]\n]+\]\(([^)\s]+)\)").unwrap())
        .captures_iter(body)
        .map(|cap| cap[1].to_string())
        .collect()
}

fn markdown_link_labels(body: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]\n]+)\]\([^)]+\)").unwrap())
        .captures_iter(body)
        .map(|cap| plain_link_label(&cap[1]))
        .collect()
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

fn write_file(root: &Path, rel: &Path, body: &str) -> Result<()> {
    let abs = root.join(rel);
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&abs, body).with_context(|| format!("failed to write {}", abs.display()))?;
    Ok(())
}

fn escape_table(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_agent_guide_catalog_and_terms() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("fragments/a")).unwrap();
        fs::write(
            root.join("index.md"),
            "# Wiki\n\n- [Notes](fragments/_index.md)\n- [Agent](agent/index.md)\n",
        )
        .unwrap();
        fs::write(root.join("fragments/_index.md"), "# fragments\n\n").unwrap();
        fs::write(root.join("fragments/a/index.md"), "# Alpha\n\n## Design\n").unwrap();
        fs::create_dir_all(root.join("headings")).unwrap();
        fs::write(root.join("headings/index.md"), "# Headings\n").unwrap();
        fs::create_dir_all(root.join("links")).unwrap();
        fs::write(root.join("links/index.md"), "# Links\n").unwrap();
        fs::create_dir_all(root.join("tags")).unwrap();
        fs::write(root.join("tags/index.md"), "# Tags\n").unwrap();
        fs::write(root.join("_unresolved.md"), "# Unresolved\n").unwrap();

        finalize_agentic_output(root).unwrap();

        assert!(root.join("agent/index.md").exists());
        assert!(root.join("agent/pages/index.md").exists());
        assert!(root.join("agent/terms/index.md").exists());
        let page = fs::read_to_string(root.join("fragments/a/index.md")).unwrap();
        assert!(page.starts_with("---\nmd_wiki:\n"));
        let terms = fs::read_to_string(root.join("agent/terms/index.md")).unwrap();
        assert!(terms.contains("design"));
    }

    #[test]
    fn paged_index_manifest_describes_page_ranges() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("headings")).unwrap();
        let mut body = String::from("# Headings\n\n");
        for idx in 0..1400 {
            let _ = writeln!(
                body,
                "## [Note {idx}](../fragments/note-{idx}/index.md)\n\n- [Heading {idx}](../fragments/note-{idx}/part.md)\n"
            );
        }
        fs::write(root.join("headings/index.md"), body).unwrap();

        finalize_agentic_output(root).unwrap();

        let manifest = fs::read_to_string(root.join("headings/index.md")).unwrap();
        assert!(manifest.contains("- [Page 1](page-001.md) — "));
        assert!(manifest.contains(" headings, `Note 0` ... `"));
        assert!(manifest.contains("Heading 1399"));
    }

    #[test]
    fn page_catalog_manifest_describes_path_ranges() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("fragments")).unwrap();
        fs::write(root.join("index.md"), "# Wiki\n").unwrap();
        for idx in 0..700 {
            let dir = root.join(format!("fragments/source-{idx:03}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("index.md"),
                format!("# Source {idx}\n\nbody with enough catalog text {idx}\n"),
            )
            .unwrap();
        }

        finalize_agentic_output(root).unwrap();

        let catalog = fs::read_to_string(root.join("agent/pages/index.md")).unwrap();
        assert!(catalog.contains("- [Page 1](page-001.md) — "));
        assert!(catalog.contains("`agent/index.md` to `"));
    }

    #[test]
    fn page_catalog_writes_by_source_lookup() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("fragments/story/world")).unwrap();
        fs::write(
            root.join("fragments/story/world/index.md"),
            "---\nmd_wiki:\n  page_kind: entry\n  output_path: \"fragments/story/world/index.md\"\n  title: \"World\"\n  source: \"story/world.md\"\n  section_path:\n    - \"fragments\"\n    - \"story\"\n    - \"world\"\n  char_count: 8\n  parent:\n  prev:\n  next:\n  children:\n  backlinks_count: 0\n---\n# World\n",
        )
        .unwrap();
        fs::write(
            root.join("fragments/story/world/nations.md"),
            "---\nmd_wiki:\n  page_kind: leaf\n  output_path: \"fragments/story/world/nations.md\"\n  title: \"Nations\"\n  source: \"story/world.md\"\n  section_path:\n    - \"fragments\"\n    - \"story\"\n    - \"world\"\n  char_count: 10\n  parent: \"index.md\"\n  prev:\n  next:\n  children:\n  backlinks_count: 0\n---\n## Nations\n",
        )
        .unwrap();

        finalize_agentic_output(root).unwrap();

        let index = fs::read_to_string(root.join("agent/pages/by-source/index.md")).unwrap();
        assert!(index.contains("- [story/world.md](story_world.md) — 2 pages"));
        let source = fs::read_to_string(root.join("agent/pages/by-source/story_world.md")).unwrap();
        assert!(source.contains("# story/world.md"));
        assert!(source.contains("fragments/story/world/index.md"));
        assert!(source.contains("fragments/story/world/nations.md"));
    }

    #[test]
    fn term_index_suppresses_generated_index_noise() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("headings")).unwrap();
        fs::write(
            root.join("headings/index.md"),
            "# Headings\n\n- [Page 1](page-001.md)\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("fragments/topic")).unwrap();
        fs::write(
            root.join("fragments/topic/index.md"),
            "# Topic\n\n## Useful Term\n",
        )
        .unwrap();

        finalize_agentic_output(root).unwrap();

        let terms = fs::read_to_string(root.join("agent/terms/index.md")).unwrap();
        assert!(terms.contains("useful term"));
        assert!(!terms.contains("| page-001 |"));
        assert!(!terms.contains("| index |"));
        assert!(!terms.contains("| headings |"));
    }
}
