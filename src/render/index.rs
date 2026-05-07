use std::fmt::Write;
use std::path::Component;

use super::tags::TagIndex;
use crate::link::UnresolvedLink;
use crate::model::{Node, PageKind, iter_pages};

/// ルート `index.md` の本文を生成する（FR-12 / AC-22）。
/// サイト全体のサマリと各索引への導線のみを置く。ノート一覧は `fragments/_index.md` に委譲。
pub fn render_index(
    project_title: &str,
    nodes: &[Node],
    unresolved: &[UnresolvedLink],
    tag_index: &TagIndex,
) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# {project_title}");
    s.push('\n');

    let fragment_count: usize = nodes
        .iter()
        .flat_map(iter_pages)
        .filter(|p| matches!(p.kind, PageKind::H2Leaf | PageKind::H3Leaf))
        .count();

    let _ = writeln!(&mut s, "## Summary");
    s.push('\n');
    let _ = writeln!(&mut s, "- Notes: {}", nodes.len());
    let _ = writeln!(&mut s, "- Fragments: {fragment_count}");
    let _ = writeln!(&mut s, "- Tags: {}", tag_index.entries.len());
    let _ = writeln!(&mut s, "- Unresolved links: {}", unresolved.len());
    s.push('\n');

    let _ = writeln!(&mut s, "## Sections");
    s.push('\n');
    let _ = writeln!(&mut s, "- [Agent Guide](agent/index.md)");
    let _ = writeln!(&mut s, "- [Notes](fragments/_index.md)");
    let _ = writeln!(&mut s, "- [Tags](tags/index.md)");
    let _ = writeln!(&mut s, "- [Headings](headings/index.md)");
    let _ = writeln!(&mut s, "- [Links](links/index.md)");
    s.push('\n');

    let _ = writeln!(&mut s, "## Search Map");
    s.push('\n');
    let _ = writeln!(&mut s, "| Need | Start here |");
    let _ = writeln!(&mut s, "|---|---|");
    let _ = writeln!(
        &mut s,
        "| Agent workflow and reading rules | [Agent Guide](agent/index.md) |"
    );
    let _ = writeln!(
        &mut s,
        "| Page inventory, source paths, and ranges | [Page Catalog](agent/pages/index.md) |"
    );
    let _ = writeln!(
        &mut s,
        "| Terms from titles, headings, tags, files, and links | [Term Index](agent/terms/index.md) |"
    );
    let _ = writeln!(
        &mut s,
        "| Browse note hierarchy | [Notes](fragments/_index.md) |"
    );
    let _ = writeln!(
        &mut s,
        "| Find h1/h2 headings | [Headings](headings/index.md) |"
    );
    let _ = writeln!(
        &mut s,
        "| Inspect forward links and backlinks | [Links](links/index.md) |"
    );
    let _ = writeln!(&mut s, "| Browse tags | [Tags](tags/index.md) |");
    let _ = writeln!(
        &mut s,
        "| Check unresolved wikilinks | [_unresolved.md](_unresolved.md) |"
    );
    s.push('\n');

    push_contents_preview(&mut s, nodes);

    if !unresolved.is_empty() {
        let _ = writeln!(
            &mut s,
            "## Unresolved links\n\n未解決の wikilink が {} 件あります。詳細は [_unresolved.md](_unresolved.md)。",
            unresolved.len()
        );
        s.push('\n');
    }
    s
}

#[derive(Default)]
struct PreviewBucket {
    notes: usize,
    fragments: usize,
}

fn push_contents_preview(out: &mut String, nodes: &[Node]) {
    let _ = writeln!(out, "## Contents Preview");
    out.push('\n');
    let _ = writeln!(out, "### Notes");
    out.push('\n');

    let mut buckets: std::collections::BTreeMap<String, PreviewBucket> =
        std::collections::BTreeMap::new();
    let mut root_notes = PreviewBucket::default();
    for n in nodes {
        let fragments = iter_pages(n)
            .into_iter()
            .filter(|p| matches!(p.kind, PageKind::H2Leaf | PageKind::H3Leaf))
            .count();
        if let Some(top) = top_source_dir(&n.note.source_file) {
            let bucket = buckets.entry(top).or_default();
            bucket.notes += 1;
            bucket.fragments += fragments;
        } else {
            root_notes.notes += 1;
            root_notes.fragments += fragments;
        }
    }

    if buckets.is_empty() && root_notes.notes == 0 {
        let _ = writeln!(out, "- _(no notes)_");
    }
    if root_notes.notes > 0 {
        let _ = writeln!(
            out,
            "- Root notes — {} notes, {} fragments",
            root_notes.notes, root_notes.fragments
        );
    }
    for (name, bucket) in buckets.iter().take(12) {
        let _ = writeln!(
            out,
            "- [{name}](fragments/{name}/_index.md) — {} notes, {} fragments",
            bucket.notes, bucket.fragments
        );
    }
    if buckets.len() > 12 {
        let _ = writeln!(out, "- ... {} more directories", buckets.len() - 12);
    }
    out.push('\n');

    let _ = writeln!(out, "### Headings Preview");
    out.push('\n');
    let mut headings = Vec::new();
    for n in nodes {
        for heading in &n.note.headings {
            if heading.level <= 2 {
                headings.push(heading.text.clone());
            }
            if headings.len() >= 12 {
                break;
            }
        }
        if headings.len() >= 12 {
            break;
        }
    }
    if headings.is_empty() {
        let _ = writeln!(out, "- _(no headings)_");
    } else {
        for heading in headings {
            let _ = writeln!(out, "- {heading}");
        }
    }
    out.push('\n');
}

fn top_source_dir(source_file: &std::path::Path) -> Option<String> {
    source_file
        .components()
        .find_map(|component| match component {
            Component::Normal(part)
                if source_file
                    .parent()
                    .is_some_and(|p| !p.as_os_str().is_empty()) =>
            {
                Some(part.to_string_lossy().into_owned())
            }
            _ => None,
        })
}
