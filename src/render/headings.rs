use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use super::paths::{fragment_leaf_path, relative_link, shell_index_path};
use super::text::link_label;
use crate::fragment::Fragment;
use crate::link::slugify;
use crate::model::Node;

/// `headings/index.md` の本文を生成する（FR-08）。
/// h1 は入口ページ、h2 は対応する断片ページ（非断片化ノートは入口 `#slug`）へ張る。
pub fn render_headings_index(nodes: &[Node]) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Headings");
    s.push('\n');

    let from = PathBuf::from("headings/index.md");
    let mut any_heading = false;

    for n in nodes {
        let entries = heading_entries(n, &from);
        if entries.is_empty() {
            continue;
        }
        any_heading = true;
        let note_link = relative_link(&from, &n.output_path);
        let _ = writeln!(&mut s, "## [{}]({})", link_label(&n.title), note_link);
        s.push('\n');
        for entry in entries {
            let indent = if entry.level == 2 { "  " } else { "" };
            let _ = writeln!(
                &mut s,
                "{indent}- [{}]({})",
                link_label(&entry.text),
                entry.link
            );
        }
        s.push('\n');
    }

    if !any_heading {
        let _ = writeln!(&mut s, "_(no headings)_");
    }
    s
}

struct HeadingEntry {
    level: u8,
    text: String,
    link: String,
}

fn heading_entries(n: &Node, from: &Path) -> Vec<HeadingEntry> {
    let mut out = Vec::new();
    if let Some(h1) = n.note.headings.iter().find(|h| h.level == 1) {
        out.push(HeadingEntry {
            level: 1,
            text: h1.text.clone(),
            link: relative_link(from, &n.output_path),
        });
    }
    if n.fragments.non_fragmented {
        let mut used: HashMap<String, usize> = HashMap::new();
        for h in &n.note.headings {
            if h.level != 2 {
                continue;
            }
            let slug = disambiguate(&slugify(&h.text), &mut used);
            let entry_link = relative_link(from, &n.output_path);
            out.push(HeadingEntry {
                level: 2,
                text: h.text.clone(),
                link: format!("{entry_link}#{slug}"),
            });
        }
    } else {
        for frag in &n.fragments.fragments {
            let (heading, target) = match frag {
                Fragment::H2 { heading, slug, .. } => {
                    (heading.clone(), fragment_leaf_path(&n.entry_dir, slug))
                }
                Fragment::Shell { heading, slug, .. } => {
                    (heading.clone(), shell_index_path(&n.entry_dir, slug))
                }
            };
            out.push(HeadingEntry {
                level: 2,
                text: heading,
                link: relative_link(from, &target),
            });
        }
    }
    out
}

fn disambiguate(base: &str, used: &mut HashMap<String, usize>) -> String {
    if let Some(count) = used.get_mut(base) {
        *count += 1;
        format!("{base}-{count}")
    } else {
        used.insert(base.to_string(), 0);
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::build_fragments;
    use crate::notes::{Frontmatter, NoteData, headings};

    fn fragmented_node(src: &str, body: &str) -> Node {
        let source = PathBuf::from(src);
        let entry = crate::render::paths::entry_index_path(&source);
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
            title: "n".into(),
            note,
            fragments,
            related: vec![],
            backlinks: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn h2_links_to_fragment_page() {
        let n = fragmented_node("foo.md", "# Foo\n\n## Intro\n\nhi\n");
        let body = render_headings_index(&[n]);
        assert!(body.contains("../fragments/foo/intro.md"));
        // h1 は入口ページ
        assert!(body.contains("../fragments/foo/index.md"));
    }

    #[test]
    fn non_fragmented_note_uses_anchor() {
        let mut n = fragmented_node("foo.md", "# Foo\n\n## Intro\n");
        // h2 が 1 個なので断片化されるはず。意図的に `fragment: false` をかける
        n.note.frontmatter.fragment = Some(false);
        n.fragments = build_fragments(&n.note);
        let body = render_headings_index(&[n]);
        assert!(body.contains("../fragments/foo/index.md#intro"));
    }
}
