use std::fmt::Write;
use std::path::{Path, PathBuf};

use crate::large_markdown::{ByteRange, LineRange, SplitReason};

#[derive(Debug, Clone)]
pub struct Metadata {
    pub page_kind: String,
    pub output_path: PathBuf,
    pub title: String,
    pub source: Option<PathBuf>,
    pub section_path: Vec<String>,
    pub heading_level: Option<u8>,
    pub split_reason: Option<SplitReason>,
    pub char_count: usize,
    pub byte_ranges: Vec<ByteRange>,
    pub line_ranges: Vec<LineRange>,
    pub parent: Option<PathBuf>,
    pub prev: Option<PathBuf>,
    pub next: Option<PathBuf>,
    pub children: Vec<PathBuf>,
    pub tags: Vec<String>,
    pub outgoing_links: Vec<PathBuf>,
    pub backlinks_count: usize,
}

pub fn render_frontmatter(meta: &Metadata) -> String {
    let mut out = String::from("---\nmd_wiki:\n");
    let _ = writeln!(out, "  schema_version: 1");
    let _ = writeln!(out, "  page_kind: {}", meta.page_kind);
    let _ = writeln!(
        out,
        "  output_path: {}",
        yaml_scalar(&meta.output_path.display().to_string())
    );
    let _ = writeln!(out, "  title: {}", yaml_scalar(&meta.title));
    if let Some(source) = &meta.source {
        let _ = writeln!(
            out,
            "  source: {}",
            yaml_scalar(&source.display().to_string())
        );
    }
    out.push_str("  section_path:\n");
    for section in &meta.section_path {
        let _ = writeln!(out, "    - {}", yaml_scalar(section));
    }
    if let Some(level) = meta.heading_level {
        let _ = writeln!(out, "  heading_level: {level}");
    }
    if let Some(reason) = meta.split_reason {
        let _ = writeln!(out, "  split_reason: {}", split_reason_name(reason));
    }
    let _ = writeln!(out, "  char_count: {}", meta.char_count);
    if !meta.byte_ranges.is_empty() {
        out.push_str("  byte_ranges:\n");
        for range in &meta.byte_ranges {
            let _ = writeln!(out, "    - [{}, {}]", range.start, range.end);
        }
    }
    if !meta.line_ranges.is_empty() {
        out.push_str("  line_ranges:\n");
        for range in &meta.line_ranges {
            let _ = writeln!(out, "    - [{}, {}]", range.start, range.end);
        }
    }
    render_optional_path(&mut out, "parent", meta.parent.as_deref());
    render_optional_path(&mut out, "prev", meta.prev.as_deref());
    render_optional_path(&mut out, "next", meta.next.as_deref());
    out.push_str("  children:\n");
    for child in &meta.children {
        let _ = writeln!(out, "    - {}", yaml_scalar(&child.display().to_string()));
    }
    if !meta.tags.is_empty() {
        out.push_str("  tags:\n");
        for tag in &meta.tags {
            let _ = writeln!(out, "    - {}", yaml_scalar(tag));
        }
    }
    if !meta.outgoing_links.is_empty() {
        out.push_str("  outgoing_links:\n");
        for link in &meta.outgoing_links {
            let _ = writeln!(out, "    - {}", yaml_scalar(&link.display().to_string()));
        }
    }
    let _ = writeln!(out, "  backlinks_count: {}", meta.backlinks_count);
    out.push_str("---\n");
    out
}

fn render_optional_path(out: &mut String, key: &str, value: Option<&Path>) {
    match value {
        Some(path) => {
            let _ = writeln!(out, "  {key}: {}", yaml_scalar(&path.display().to_string()));
        }
        None => {
            let _ = writeln!(out, "  {key}:");
        }
    }
}

pub fn yaml_scalar(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn split_reason_name(reason: SplitReason) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_parseable_md_wiki_frontmatter_shape() {
        let body = render_frontmatter(&Metadata {
            page_kind: "leaf".into(),
            output_path: PathBuf::from("fragments/a/part-001.md"),
            title: "Part 1".into(),
            source: Some(PathBuf::from("a.md")),
            section_path: vec!["A".into()],
            heading_level: Some(2),
            split_reason: Some(SplitReason::Heading),
            char_count: 12,
            byte_ranges: vec![ByteRange { start: 0, end: 12 }],
            line_ranges: vec![LineRange { start: 1, end: 2 }],
            parent: Some(PathBuf::from("index.md")),
            prev: None,
            next: None,
            children: Vec::new(),
            tags: Vec::new(),
            outgoing_links: Vec::new(),
            backlinks_count: 0,
        });
        assert!(body.starts_with("---\nmd_wiki:\n"));
        assert!(body.contains("  page_kind: leaf\n"));
        assert!(body.contains("  byte_ranges:\n"));
    }
}
