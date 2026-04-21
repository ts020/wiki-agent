use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use crate::model::{Node, PageKind, iter_pages};

/// 1 つの `_index.md` 出力単位。
pub struct SiteIndexPage {
    pub output_path: PathBuf,
    pub body: String,
}

/// `fragments/` 配下の各ディレクトリに対し `_index.md` を生成する（FR-16）。
pub fn render_site_indexes(nodes: &[Node]) -> Vec<SiteIndexPage> {
    let tree = build_tree(nodes);
    tree.iter()
        .map(|(dir, info)| SiteIndexPage {
            output_path: dir.join("_index.md"),
            body: render_page(dir, info, &tree, nodes),
        })
        .collect()
}

#[derive(Default)]
struct SectionInfo {
    direct_notes: BTreeSet<PathBuf>,
    direct_subsections: BTreeSet<PathBuf>,
    recursive_note_count: usize,
    recursive_fragment_count: usize,
}

fn fragments_root() -> &'static Path {
    Path::new("fragments")
}

fn build_tree(nodes: &[Node]) -> BTreeMap<PathBuf, SectionInfo> {
    let entry_dirs: BTreeSet<PathBuf> = nodes.iter().map(|n| n.entry_dir.clone()).collect();

    let mut sections: BTreeSet<PathBuf> = BTreeSet::new();
    sections.insert(fragments_root().to_path_buf());
    for ed in &entry_dirs {
        collect_ancestors(ed, &mut sections);
    }

    let mut tree: BTreeMap<PathBuf, SectionInfo> = sections
        .iter()
        .map(|s| (s.clone(), SectionInfo::default()))
        .collect();

    for ed in &entry_dirs {
        if let Some(parent) = ed.parent()
            && let Some(info) = tree.get_mut(parent)
        {
            info.direct_notes.insert(ed.clone());
        }
    }

    let section_list: Vec<PathBuf> = sections.iter().cloned().collect();
    for sec in &section_list {
        if sec == fragments_root() {
            continue;
        }
        if let Some(parent) = sec.parent()
            && let Some(info) = tree.get_mut(parent)
        {
            info.direct_subsections.insert(sec.clone());
        }
    }

    for n in nodes {
        let frag_count = iter_pages(n)
            .iter()
            .filter(|p| matches!(p.kind, PageKind::H2Leaf | PageKind::H3Leaf))
            .count();
        let mut ancestors: BTreeSet<PathBuf> = BTreeSet::new();
        collect_ancestors(&n.entry_dir, &mut ancestors);
        for a in &ancestors {
            if let Some(info) = tree.get_mut(a) {
                info.recursive_note_count += 1;
                info.recursive_fragment_count += frag_count;
            }
        }
    }

    tree
}

/// `entry_dir` から `fragments/` までの祖先ディレクトリを集める（`fragments/` 含む、`entry_dir` 自体は含まない）。
fn collect_ancestors(entry_dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let mut cur = entry_dir.parent().map(Path::to_path_buf);
    while let Some(p) = cur {
        if p.as_os_str().is_empty() {
            break;
        }
        out.insert(p.clone());
        if p == fragments_root() {
            break;
        }
        cur = p.parent().map(Path::to_path_buf);
    }
}

fn render_page(
    dir: &Path,
    info: &SectionInfo,
    tree: &BTreeMap<PathBuf, SectionInfo>,
    nodes: &[Node],
) -> String {
    let mut s = String::new();
    let heading = if dir == fragments_root() {
        "fragments".to_string()
    } else {
        dir.to_string_lossy().into_owned()
    };
    let _ = writeln!(&mut s, "# {heading}");
    s.push('\n');

    let _ = writeln!(&mut s, "## Summary");
    s.push('\n');
    let _ = writeln!(&mut s, "- Notes: {} (recursive)", info.recursive_note_count);
    let _ = writeln!(
        &mut s,
        "- Fragments: {} (recursive)",
        info.recursive_fragment_count
    );
    let _ = writeln!(
        &mut s,
        "- Subdirectories: {}",
        info.direct_subsections.len()
    );
    s.push('\n');

    if !info.direct_subsections.is_empty() {
        let _ = writeln!(&mut s, "## Subdirectories");
        s.push('\n');
        for sub in &info.direct_subsections {
            let name = sub
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let sub_info = tree.get(sub).expect("subsection tracked in tree");
            let _ = writeln!(
                &mut s,
                "- [{name}]({name}/_index.md) — {} ノート",
                sub_info.recursive_note_count
            );
        }
        s.push('\n');
    }

    if !info.direct_notes.is_empty() {
        let _ = writeln!(&mut s, "## Notes");
        s.push('\n');
        for note_dir in &info.direct_notes {
            let name = note_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let title = nodes
                .iter()
                .find(|n| n.entry_dir == *note_dir)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| name.clone());
            let _ = writeln!(&mut s, "- [{title}]({name}/index.md)");
        }
        s.push('\n');
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::build_fragments;
    use crate::notes::{Frontmatter, NoteData, headings};
    use crate::render::paths::entry_index_path;

    fn make_node(src: &str, body: &str, title: &str) -> Node {
        let source = PathBuf::from(src);
        let entry = entry_index_path(&source);
        let entry_dir = entry.parent().unwrap().to_path_buf();
        let note = NoteData {
            source_file: source,
            frontmatter: Frontmatter::default(),
            headings: headings::extract(body),
            first_paragraph: None,
            body: body.to_string(),
        };
        let fragments = build_fragments(&note);
        Node {
            output_path: entry,
            entry_dir,
            title: title.to_string(),
            note,
            fragments,
            related: vec![],
            backlinks: BTreeMap::new(),
        }
    }

    fn find_page<'a>(pages: &'a [SiteIndexPage], rel: &str) -> Option<&'a SiteIndexPage> {
        pages.iter().find(|p| p.output_path == Path::new(rel))
    }

    #[test]
    fn empty_input_emits_fragments_index_only() {
        let pages = render_site_indexes(&[]);
        assert_eq!(pages.len(), 1);
        let root = &pages[0];
        assert_eq!(root.output_path, PathBuf::from("fragments/_index.md"));
        assert!(root.body.contains("# fragments"));
        assert!(root.body.contains("- Notes: 0 (recursive)"));
    }

    #[test]
    fn flat_notes_listed_under_fragments() {
        let n1 = make_node("memo.md", "# Memo\n\nbody\n", "Memo");
        let n2 = make_node("todo.md", "# Todo\n\n## A\n\nx\n", "Todo");
        let pages = render_site_indexes(&[n1, n2]);
        let root = find_page(&pages, "fragments/_index.md").expect("fragments root");
        assert!(root.body.contains("- Notes: 2 (recursive)"));
        assert!(root.body.contains("- Fragments: 1 (recursive)"));
        assert!(root.body.contains("- Subdirectories: 0"));
        assert!(root.body.contains("- [Memo](memo/index.md)"));
        assert!(root.body.contains("- [Todo](todo/index.md)"));
    }

    #[test]
    fn nested_tree_builds_section_chain() {
        let n = make_node("docs/auth/session.md", "# Session\n\nbody\n", "Session");
        let pages = render_site_indexes(&[n]);
        let root = find_page(&pages, "fragments/_index.md").expect("root");
        let docs = find_page(&pages, "fragments/docs/_index.md").expect("docs");
        let auth = find_page(&pages, "fragments/docs/auth/_index.md").expect("auth");

        assert!(root.body.contains("- [docs](docs/_index.md) — 1 ノート"));
        assert!(root.body.contains("- Subdirectories: 1"));
        assert!(!root.body.contains("## Notes"));

        assert!(docs.body.contains("- [auth](auth/_index.md) — 1 ノート"));
        assert!(auth.body.contains("- [Session](session/index.md)"));
        assert!(auth.body.contains("- Notes: 1 (recursive)"));
    }

    #[test]
    fn note_and_subdirectory_coexist_at_same_level() {
        let n1 = make_node("root.md", "# Root\n\nx\n", "Root");
        let n2 = make_node("sub/child.md", "# Child\n\ny\n", "Child");
        let pages = render_site_indexes(&[n1, n2]);
        let root = find_page(&pages, "fragments/_index.md").expect("root");
        assert!(root.body.contains("- [Root](root/index.md)"));
        assert!(root.body.contains("- [sub](sub/_index.md) — 1 ノート"));
        assert!(root.body.contains("- Subdirectories: 1"));
        assert!(root.body.contains("- Notes: 2 (recursive)"));
    }
}
