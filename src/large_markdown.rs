use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::link::slugify;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRecord {
    pub byte_start: u64,
    pub byte_end: u64,
    pub line_number: u64,
    pub char_count: u32,
    pub kind_hint: LineKindHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKindHint {
    Plain,
    Blank,
    Heading,
    Fence,
    Table,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineScanReport {
    pub lines: Vec<LineRecord>,
    pub bytes: u64,
    pub max_buffered_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionTree {
    pub sections: Vec<SectionNode>,
    pub heading_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub level: u8,
    pub title: String,
    pub slug: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub line_start: u64,
    pub line_end: u64,
    pub children: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Entry,
    Shell,
    Leaf,
    PagedIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitReason {
    Heading,
    Paragraph,
    List,
    Table,
    CodeFence,
    LineWindow,
    ByteWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagePlan {
    pub page_id: String,
    pub page_kind: PageKind,
    pub output_path: PathBuf,
    pub source_path: Option<PathBuf>,
    pub section_path: Vec<String>,
    pub byte_ranges: Vec<ByteRange>,
    pub line_ranges: Vec<LineRange>,
    pub split_reason: SplitReason,
    pub parent: Option<PathBuf>,
    pub prev: Option<PathBuf>,
    pub next: Option<PathBuf>,
    pub estimated_chars: usize,
}

pub fn scan_lines(_path: &Path) -> Result<LineScanReport> {
    let file = File::open(_path).with_context(|| format!("failed to open {}", _path.display()))?;
    let mut reader = BufReader::new(file);
    let mut buf = Vec::new();
    let mut lines = Vec::new();
    let mut byte_start = 0u64;
    let mut line_number = 1u64;
    let mut max_buffered_bytes = 0usize;

    loop {
        buf.clear();
        let read = reader
            .read_until(b'\n', &mut buf)
            .with_context(|| format!("failed to read {}", _path.display()))?;
        if read == 0 {
            break;
        }
        max_buffered_bytes = max_buffered_bytes.max(buf.len());
        if buf.contains(&0) {
            bail!(
                "NULL byte found while scanning {} at line {}",
                _path.display(),
                line_number
            );
        }
        let text = std::str::from_utf8(&buf).with_context(|| {
            format!(
                "invalid UTF-8 while scanning {} at line {}",
                _path.display(),
                line_number
            )
        })?;
        let byte_end = byte_start + read as u64;
        lines.push(LineRecord {
            byte_start,
            byte_end,
            line_number,
            char_count: text.chars().count() as u32,
            kind_hint: classify_line(text),
        });
        byte_start = byte_end;
        line_number += 1;
    }

    Ok(LineScanReport {
        lines,
        bytes: byte_start,
        max_buffered_bytes,
    })
}

pub fn plan_leaf_pages(
    source_path: &Path,
    line_report: &LineScanReport,
    section_tree: &SectionTree,
    hard_limit: usize,
) -> Vec<PagePlan> {
    let entry_slug = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(slugify)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "note".into());
    let entry_dir = PathBuf::from("fragments").join(&entry_slug);
    let source_label = source_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("source.md")
        .to_string();
    let mut plans = vec![PagePlan {
        page_id: format!("{entry_slug}:entry"),
        page_kind: PageKind::Entry,
        output_path: entry_dir.join("index.md"),
        source_path: Some(source_path.to_path_buf()),
        section_path: vec![source_label.clone()],
        byte_ranges: Vec::new(),
        line_ranges: Vec::new(),
        split_reason: SplitReason::Heading,
        parent: None,
        prev: None,
        next: None,
        estimated_chars: 0,
    }];

    let chunk_limit = hard_limit.saturating_sub(10_000).max(1_000);
    let split_reason = choose_split_reason(line_report, section_tree);
    let chunks = chunk_lines(&line_report.lines, chunk_limit);
    let chunk_count = chunks.len();
    for (idx, chunk) in chunks.into_iter().enumerate() {
        let page_name = format!("part-{number:03}.md", number = idx + 1);
        let prev = (idx > 0).then(|| PathBuf::from(format!("part-{number:03}.md", number = idx)));
        let next = (idx + 1 < chunk_count)
            .then(|| PathBuf::from(format!("part-{number:03}.md", number = idx + 2)));
        plans.push(PagePlan {
            page_id: format!("{entry_slug}:part-{number:03}", number = idx + 1),
            page_kind: PageKind::Leaf,
            output_path: entry_dir.join(&page_name),
            source_path: Some(source_path.to_path_buf()),
            section_path: vec![source_label.clone(), format!("Part {}", idx + 1)],
            byte_ranges: vec![ByteRange {
                start: chunk.byte_start,
                end: chunk.byte_end,
            }],
            line_ranges: vec![LineRange {
                start: chunk.line_start,
                end: chunk.line_end,
            }],
            split_reason,
            parent: Some(PathBuf::from("index.md")),
            prev,
            next,
            estimated_chars: chunk.chars,
        });
    }

    plans
}

#[derive(Debug, Clone, Copy)]
struct Chunk {
    byte_start: u64,
    byte_end: u64,
    line_start: u64,
    line_end: u64,
    chars: usize,
}

fn chunk_lines(lines: &[LineRecord], chunk_limit: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current: Option<Chunk> = None;

    for line in lines {
        let line_chars = line.char_count as usize;
        if let Some(chunk) = &mut current
            && chunk.chars > 0
            && chunk.chars + line_chars > chunk_limit
        {
            chunks.push(*chunk);
            current = None;
        }
        let chunk = current.get_or_insert(Chunk {
            byte_start: line.byte_start,
            byte_end: line.byte_end,
            line_start: line.line_number,
            line_end: line.line_number,
            chars: 0,
        });
        chunk.byte_end = line.byte_end;
        chunk.line_end = line.line_number;
        chunk.chars += line_chars;
    }

    if let Some(chunk) = current {
        chunks.push(chunk);
    }
    chunks
}

fn choose_split_reason(line_report: &LineScanReport, section_tree: &SectionTree) -> SplitReason {
    if line_report
        .lines
        .iter()
        .any(|line| line.kind_hint == LineKindHint::Fence)
    {
        return SplitReason::CodeFence;
    }
    if line_report
        .lines
        .iter()
        .any(|line| line.kind_hint == LineKindHint::Table)
    {
        return SplitReason::Table;
    }
    if section_tree.heading_count > 1 {
        return SplitReason::Heading;
    }
    if section_tree.heading_count == 0 {
        return SplitReason::Paragraph;
    }
    SplitReason::LineWindow
}

pub fn scan_section_tree(_path: &Path) -> Result<SectionTree> {
    let file = File::open(_path).with_context(|| format!("failed to open {}", _path.display()))?;
    let mut reader = BufReader::new(file);
    let mut buf = Vec::new();
    let mut sections = vec![SectionNode {
        id: "root".into(),
        parent_id: None,
        level: 0,
        title: "root".into(),
        slug: "root".into(),
        byte_start: 0,
        byte_end: 0,
        line_start: 1,
        line_end: 0,
        children: Vec::new(),
    }];
    let mut stack = vec![0usize];
    let mut heading_counts = [0usize; 7];
    let mut byte_start = 0u64;
    let mut line_number = 1u64;
    let mut last_line = 0u64;
    let mut in_fence = false;

    loop {
        buf.clear();
        let read = reader
            .read_until(b'\n', &mut buf)
            .with_context(|| format!("failed to read {}", _path.display()))?;
        if read == 0 {
            break;
        }
        if buf.contains(&0) {
            bail!(
                "NULL byte found while scanning {} at line {}",
                _path.display(),
                line_number
            );
        }
        let text = std::str::from_utf8(&buf).with_context(|| {
            format!(
                "invalid UTF-8 while scanning {} at line {}",
                _path.display(),
                line_number
            )
        })?;
        let line_start = byte_start;
        let line_end = byte_start + read as u64;
        let kind = classify_line(text);

        if !in_fence && let Some((level, title)) = parse_heading(text) {
            while sections[*stack.last().unwrap()].level >= level {
                let closed = stack.pop().unwrap();
                close_section(
                    &mut sections[closed],
                    line_start,
                    line_number.saturating_sub(1),
                );
            }

            heading_counts[level as usize] += 1;
            let mut slug = slugify(&title);
            if slug.is_empty() {
                slug = "section".into();
            }
            let id = format!("h{level}-{}-{slug}", heading_counts[level as usize]);
            let parent = *stack.last().unwrap();
            sections[parent].children.push(id.clone());
            sections.push(SectionNode {
                id: id.clone(),
                parent_id: Some(sections[parent].id.clone()),
                level,
                title,
                slug,
                byte_start: line_start,
                byte_end: 0,
                line_start: line_number,
                line_end: 0,
                children: Vec::new(),
            });
            stack.push(sections.len() - 1);
        }

        if kind == LineKindHint::Fence {
            in_fence = !in_fence;
        }
        byte_start = line_end;
        last_line = line_number;
        line_number += 1;
    }

    for section in &mut sections {
        if section.byte_end == 0 {
            close_section(section, byte_start, last_line);
        }
    }
    let heading_count = sections.len().saturating_sub(1);
    Ok(SectionTree {
        sections,
        heading_count,
    })
}

fn close_section(section: &mut SectionNode, byte_end: u64, line_end: u64) {
    section.byte_end = byte_end;
    section.line_end = line_end;
}

fn classify_line(line: &str) -> LineKindHint {
    let without_eol = line.trim_end_matches(['\r', '\n']);
    let trimmed = without_eol.trim_start();
    if trimmed.is_empty() {
        return LineKindHint::Blank;
    }
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        return LineKindHint::Fence;
    }
    if is_atx_heading(trimmed) {
        return LineKindHint::Heading;
    }
    if trimmed.starts_with('|') && trimmed[1..].contains('|') {
        return LineKindHint::Table;
    }
    if is_list_item(trimmed) {
        return LineKindHint::List;
    }
    LineKindHint::Plain
}

fn is_atx_heading(trimmed: &str) -> bool {
    let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return false;
    }
    trimmed
        .as_bytes()
        .get(hashes)
        .is_some_and(|b| b.is_ascii_whitespace())
}

fn is_list_item(trimmed: &str) -> bool {
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        return true;
    }
    let Some((digits, rest)) = trimmed.split_once('.') else {
        return false;
    };
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) && rest.starts_with(' ')
}

fn parse_heading(line: &str) -> Option<(u8, String)> {
    let without_eol = line.trim_end_matches(['\r', '\n']);
    let trimmed = without_eol.trim_start();
    if !is_atx_heading(trimmed) {
        return None;
    }
    let level = trimmed.bytes().take_while(|b| *b == b'#').count() as u8;
    let raw_title = trimmed[level as usize..].trim();
    let title = raw_title.trim_end_matches('#').trim().to_string();
    Some((level, title))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn scan_lines_records_offsets_and_kind_hints() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "# Title\n\n```rust\n| h | v |\n- item\nplain\n").unwrap();

        let report = scan_lines(file.path()).unwrap();

        assert_eq!(report.bytes, 40);
        assert_eq!(report.lines.len(), 6);
        assert_eq!(report.lines[0].byte_start, 0);
        assert_eq!(report.lines[0].byte_end, 8);
        assert_eq!(report.lines[0].line_number, 1);
        assert_eq!(report.lines[0].char_count, 8);
        assert_eq!(report.lines[0].kind_hint, LineKindHint::Heading);
        assert_eq!(report.lines[1].kind_hint, LineKindHint::Blank);
        assert_eq!(report.lines[2].kind_hint, LineKindHint::Fence);
        assert_eq!(report.lines[3].kind_hint, LineKindHint::Table);
        assert_eq!(report.lines[4].kind_hint, LineKindHint::List);
        assert_eq!(report.lines[5].kind_hint, LineKindHint::Plain);
    }

    #[test]
    fn scan_lines_handles_multimegabyte_input_with_bounded_buffer() {
        let mut file = NamedTempFile::new().unwrap();
        for i in 0..120_000 {
            writeln!(file, "line {i:06}").unwrap();
        }
        file.flush().unwrap();

        let report = scan_lines(file.path()).unwrap();

        assert!(report.bytes > 1024 * 1024);
        assert_eq!(report.lines.len(), 120_000);
        assert!(report.max_buffered_bytes < 1024);
    }

    #[test]
    fn scan_lines_rejects_null_bytes() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), b"ok\nbad\0line\n").unwrap();

        let err = scan_lines(file.path()).unwrap_err().to_string();

        assert!(err.contains("NULL byte"));
    }

    #[test]
    fn scan_section_tree_builds_nested_heading_relationships() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "# Root\n\n## First\n\n### Deep\n\n## Second\n\n```md\n# Ignored\n```\n"
        )
        .unwrap();

        let tree = scan_section_tree(file.path()).unwrap();

        assert_eq!(tree.heading_count, 4);
        assert_eq!(tree.sections.len(), 5);
        let root = &tree.sections[0];
        assert_eq!(root.children, vec!["h1-1-root"]);
        let h1 = tree.sections.iter().find(|s| s.id == "h1-1-root").unwrap();
        assert_eq!(h1.level, 1);
        assert_eq!(h1.children, vec!["h2-1-first", "h2-2-second"]);
        let h2 = tree.sections.iter().find(|s| s.id == "h2-1-first").unwrap();
        assert_eq!(h2.parent_id.as_deref(), Some("h1-1-root"));
        assert_eq!(h2.children, vec!["h3-1-deep"]);
        assert!(tree.sections.iter().all(|s| s.title != "Ignored"));
    }

    #[test]
    fn plan_leaf_pages_splits_large_inputs_under_hard_limit() {
        let mut file = NamedTempFile::new().unwrap();
        for i in 0..2_000 {
            writeln!(file, "paragraph marker {i:04} with deterministic content").unwrap();
        }
        file.flush().unwrap();

        let lines = scan_lines(file.path()).unwrap();
        let tree = scan_section_tree(file.path()).unwrap();
        let plans = plan_leaf_pages(Path::new("large-no-heading.md"), &lines, &tree, 40_000);

        let leaves: Vec<_> = plans
            .iter()
            .filter(|plan| plan.page_kind == PageKind::Leaf)
            .collect();
        assert!(leaves.len() > 1);
        assert!(
            leaves.iter().all(|plan| plan.estimated_chars <= 30_000
                && plan.split_reason == SplitReason::Paragraph)
        );
        assert_eq!(leaves[0].parent.as_deref(), Some(Path::new("index.md")));
        assert!(leaves[0].next.is_some());
        assert!(leaves.last().unwrap().prev.is_some());
    }

    #[test]
    fn plan_leaf_pages_records_forced_split_reasons() {
        let fixtures = [
            (
                "large-single-heading.md",
                "# Single\nbody\n",
                SplitReason::LineWindow,
            ),
            (
                "large-code-block.md",
                "# Code\n```text\nbody\n```\n",
                SplitReason::CodeFence,
            ),
            (
                "large-table.md",
                "# Table\n| id | v |\n|---|---|\n| 1 | x |\n",
                SplitReason::Table,
            ),
        ];

        for (name, body, expected) in fixtures {
            let mut file = NamedTempFile::new().unwrap();
            write!(file, "{body}").unwrap();
            let lines = scan_lines(file.path()).unwrap();
            let tree = scan_section_tree(file.path()).unwrap();
            let plans = plan_leaf_pages(Path::new(name), &lines, &tree, 40_000);
            assert!(
                plans
                    .iter()
                    .any(|plan| plan.page_kind == PageKind::Leaf && plan.split_reason == expected),
                "{name} should create a {expected:?} leaf"
            );
        }
    }
}
