use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::link::slugify;

const BYTE_WINDOW_CHAR_TARGET: usize = 30_000;
const READ_CHUNK_BYTES: usize = 64 * 1024;
const LINE_PREFIX_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRecord {
    pub byte_start: u64,
    pub byte_end: u64,
    pub line_number: u64,
    pub char_count: u32,
    pub kind_hint: LineKindHint,
    pub safe_split_offsets: Vec<u64>,
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

#[derive(Debug, Clone)]
struct LineStreamReport {
    lines: Vec<ScannedLine>,
    bytes: u64,
    max_buffered_bytes: usize,
}

#[derive(Debug, Clone)]
struct ScannedLine {
    record: LineRecord,
    prefix: String,
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

pub fn scan_lines(path: &Path) -> Result<LineScanReport> {
    let report = scan_physical_lines(path)?;

    Ok(LineScanReport {
        lines: report.lines.into_iter().map(|line| line.record).collect(),
        bytes: report.bytes,
        max_buffered_bytes: report.max_buffered_bytes,
    })
}

fn scan_physical_lines(path: &Path) -> Result<LineStreamReport> {
    let mut file =
        File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut read_buf = vec![0u8; READ_CHUNK_BYTES];
    let mut lines = Vec::new();
    let mut byte_offset = 0u64;
    let mut line_number = 1u64;
    let mut max_buffered_bytes = 0usize;
    let mut line = StreamingLine::new(0, line_number);

    loop {
        let read = file
            .read(&mut read_buf)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        let chunk = &read_buf[..read];
        max_buffered_bytes = max_buffered_bytes.max(read + line.buffered_bytes());

        let mut segment_start = 0usize;
        for (idx, byte) in chunk.iter().enumerate() {
            if *byte != b'\n' {
                continue;
            }
            let segment = &chunk[segment_start..=idx];
            let segment_byte_start = byte_offset + segment_start as u64;
            line.consume(segment, segment_byte_start, path)?;
            lines.push(line.finish(path)?);

            line_number += 1;
            let next_byte_start = byte_offset + idx as u64 + 1;
            line = StreamingLine::new(next_byte_start, line_number);
            segment_start = idx + 1;
        }

        if segment_start < read {
            let segment = &chunk[segment_start..];
            let segment_byte_start = byte_offset + segment_start as u64;
            line.consume(segment, segment_byte_start, path)?;
        }
        max_buffered_bytes = max_buffered_bytes.max(read + line.buffered_bytes());
        byte_offset += read as u64;
    }

    if line.has_content() {
        lines.push(line.finish(path)?);
    }

    Ok(LineStreamReport {
        lines,
        bytes: byte_offset,
        max_buffered_bytes,
    })
}

#[derive(Debug, Clone)]
struct StreamingLine {
    byte_start: u64,
    byte_end: u64,
    line_number: u64,
    char_count: usize,
    kind_prefix: String,
    safe_split_offsets: Vec<u64>,
    pending_utf8: Vec<u8>,
    pending_byte_start: u64,
    saw_content: bool,
    next_split: usize,
}

impl StreamingLine {
    fn new(byte_start: u64, line_number: u64) -> Self {
        Self {
            byte_start,
            byte_end: byte_start,
            line_number,
            char_count: 0,
            kind_prefix: String::new(),
            safe_split_offsets: Vec::new(),
            pending_utf8: Vec::new(),
            pending_byte_start: byte_start,
            saw_content: false,
            next_split: BYTE_WINDOW_CHAR_TARGET,
        }
    }

    fn buffered_bytes(&self) -> usize {
        self.pending_utf8.len() + self.kind_prefix.len()
    }

    fn has_content(&self) -> bool {
        self.saw_content || !self.pending_utf8.is_empty()
    }

    fn consume(&mut self, bytes: &[u8], absolute_start: u64, path: &Path) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.contains(&0) {
            bail!(
                "NULL byte found while scanning {} at line {}",
                path.display(),
                self.line_number
            );
        }
        self.saw_content = true;
        self.byte_end = absolute_start + bytes.len() as u64;

        if self.pending_utf8.is_empty() {
            return self.consume_utf8(bytes, absolute_start, path);
        }

        let combined_byte_start = self.pending_byte_start;
        let mut combined = Vec::with_capacity(self.pending_utf8.len() + bytes.len());
        combined.extend_from_slice(&self.pending_utf8);
        combined.extend_from_slice(bytes);
        self.pending_utf8.clear();
        self.consume_utf8(&combined, combined_byte_start, path)
    }

    fn consume_utf8(&mut self, bytes: &[u8], absolute_start: u64, path: &Path) -> Result<()> {
        match std::str::from_utf8(bytes) {
            Ok(text) => {
                self.consume_text(text, absolute_start);
                Ok(())
            }
            Err(err) if err.error_len().is_some() => {
                bail!(
                    "invalid UTF-8 while scanning {} at line {}",
                    path.display(),
                    self.line_number
                )
            }
            Err(err) => {
                let valid_up_to = err.valid_up_to();
                let (valid, pending) = bytes.split_at(valid_up_to);
                let text = std::str::from_utf8(valid).with_context(|| {
                    format!(
                        "invalid UTF-8 while scanning {} at line {}",
                        path.display(),
                        self.line_number
                    )
                })?;
                self.consume_text(text, absolute_start);
                self.pending_utf8.extend_from_slice(pending);
                self.pending_byte_start = absolute_start + valid_up_to as u64;
                Ok(())
            }
        }
    }

    fn consume_text(&mut self, text: &str, absolute_start: u64) {
        for (byte_idx, ch) in text.char_indices() {
            if self.char_count == self.next_split {
                self.safe_split_offsets
                    .push(absolute_start + byte_idx as u64);
                self.next_split += BYTE_WINDOW_CHAR_TARGET;
            }
            self.char_count += 1;
            if self.kind_prefix.len() + ch.len_utf8() <= LINE_PREFIX_BYTES {
                self.kind_prefix.push(ch);
            }
        }
    }

    fn finish(self, path: &Path) -> Result<ScannedLine> {
        if !self.pending_utf8.is_empty() {
            bail!(
                "invalid UTF-8 while scanning {} at line {}",
                path.display(),
                self.line_number
            );
        }
        let char_count = u32::try_from(self.char_count).with_context(|| {
            format!(
                "line {} in {} exceeds supported character count",
                self.line_number,
                path.display()
            )
        })?;

        let kind_hint = classify_line(&self.kind_prefix);
        Ok(ScannedLine {
            record: LineRecord {
                byte_start: self.byte_start,
                byte_end: self.byte_end,
                line_number: self.line_number,
                char_count,
                kind_hint,
                safe_split_offsets: self.safe_split_offsets,
            },
            prefix: self.kind_prefix,
        })
    }
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

    // `safe_split_offsets` are recorded at fixed `BYTE_WINDOW_CHAR_TARGET` intervals during
    // scan, so byte-window chunks can only be reduced down to that granularity. Clamp the
    // chunk limit to that floor (plus a 10k metadata reserve) so byte-window splits cannot
    // exceed the caller's hard_limit; pick a smaller `BYTE_WINDOW_CHAR_TARGET` if a tighter
    // hard_limit needs to be honoured.
    let chunk_limit = hard_limit
        .saturating_sub(10_000)
        .max(BYTE_WINDOW_CHAR_TARGET);
    let split_reason = choose_split_reason(line_report, section_tree, chunk_limit);
    let chunks = chunk_lines(&line_report.lines, chunk_limit, split_reason);
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
            split_reason: chunk.split_reason,
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
    split_reason: SplitReason,
}

fn chunk_lines(
    lines: &[LineRecord],
    chunk_limit: usize,
    default_split_reason: SplitReason,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current: Option<Chunk> = None;

    for line in lines {
        let line_chars = line.char_count as usize;
        if line_chars > chunk_limit {
            if let Some(chunk) = current.take() {
                chunks.push(chunk);
            }
            chunks.extend(byte_window_chunks(line, chunk_limit));
            continue;
        }
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
            split_reason: default_split_reason,
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

fn byte_window_chunks(line: &LineRecord, chunk_limit: usize) -> Vec<Chunk> {
    let mut boundaries = Vec::with_capacity(line.safe_split_offsets.len() + 2);
    boundaries.push(line.byte_start);
    boundaries.extend(
        line.safe_split_offsets
            .iter()
            .copied()
            .filter(|offset| *offset > line.byte_start && *offset < line.byte_end),
    );
    boundaries.push(line.byte_end);
    boundaries.dedup();

    boundaries
        .windows(2)
        .filter_map(|pair| {
            let start = pair[0];
            let end = pair[1];
            (start < end).then_some(Chunk {
                byte_start: start,
                byte_end: end,
                line_start: line.line_number,
                line_end: line.line_number,
                chars: chunk_limit.min(line.char_count as usize),
                split_reason: SplitReason::ByteWindow,
            })
        })
        .collect()
}

fn choose_split_reason(
    line_report: &LineScanReport,
    section_tree: &SectionTree,
    chunk_limit: usize,
) -> SplitReason {
    if line_report
        .lines
        .iter()
        .any(|line| line.char_count as usize > chunk_limit)
    {
        return SplitReason::ByteWindow;
    }
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

pub fn scan_section_tree(path: &Path) -> Result<SectionTree> {
    let report = scan_physical_lines(path)?;
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
    let mut last_line = 0u64;
    let mut in_fence = false;

    for scanned in &report.lines {
        let record = &scanned.record;
        let line_start = record.byte_start;
        let line_number = record.line_number;
        let kind = record.kind_hint;

        if !in_fence && let Some((level, title)) = parse_heading(&scanned.prefix) {
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
        last_line = line_number;
    }

    for section in &mut sections {
        if section.byte_end == 0 {
            close_section(section, report.bytes, last_line);
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
        assert!(report.lines[0].safe_split_offsets.is_empty());
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
        assert!(report.max_buffered_bytes < 128 * 1024);
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
    fn plan_leaf_pages_splits_single_huge_line_with_byte_windows() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", "a".repeat(95_000)).unwrap();
        file.flush().unwrap();

        let lines = scan_lines(file.path()).unwrap();
        let tree = scan_section_tree(file.path()).unwrap();
        let plans = plan_leaf_pages(Path::new("large-single-line.md"), &lines, &tree, 40_000);

        assert!(lines.max_buffered_bytes < 128 * 1024);
        let leaves: Vec<_> = plans
            .iter()
            .filter(|plan| plan.page_kind == PageKind::Leaf)
            .collect();
        assert!(leaves.len() >= 4);
        assert!(leaves.iter().all(|plan| {
            plan.estimated_chars <= 30_000 && plan.split_reason == SplitReason::ByteWindow
        }));
        assert_eq!(leaves.first().unwrap().byte_ranges[0].start, 0);
        assert_eq!(leaves.last().unwrap().byte_ranges[0].end, lines.bytes);
        for pair in leaves.windows(2) {
            assert_eq!(pair[0].byte_ranges[0].end, pair[1].byte_ranges[0].start);
            assert_eq!(pair[0].line_ranges[0].start, 1);
            assert_eq!(pair[1].line_ranges[0].start, 1);
        }
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
