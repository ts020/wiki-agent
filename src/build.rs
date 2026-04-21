use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use crate::fragment::build_fragments;
use crate::model::Node;
use crate::notes::NoteData;
use crate::render::paths::{entry_index_path, resolve_conflict};

/// 取り込んだノートから内部モデルを組み立てる（FR-05）。
/// 入口ページ `fragments/<rel>/index.md` を一意化し、断片ツリーを計算する。
pub fn build_nodes(notes: Vec<NoteData>) -> Vec<Node> {
    let mut used: HashSet<PathBuf> = HashSet::new();
    let mut out = Vec::with_capacity(notes.len());
    for data in notes {
        let entry = resolve_conflict(entry_index_path(&data.source_file), &mut used);
        let dir = entry.parent().map(PathBuf::from).unwrap_or_default();
        let title = derive_title(&data);
        let fragments = build_fragments(&data);
        out.push(Node {
            output_path: entry,
            entry_dir: dir,
            title,
            note: data,
            fragments,
            related: Vec::new(),
            backlinks: BTreeMap::new(),
        });
    }
    out
}

fn derive_title(data: &NoteData) -> String {
    if let Some(t) = &data.frontmatter.title {
        return t.clone();
    }
    if let Some(h) = data.headings.iter().find(|h| h.level == 1) {
        return h.text.clone();
    }
    data.source_file
        .file_stem()
        .and_then(|s| s.to_str())
        .map(String::from)
        .unwrap_or_else(|| data.source_file.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::Frontmatter;

    fn note(source: &str, title: Option<&str>) -> NoteData {
        NoteData {
            source_file: PathBuf::from(source),
            frontmatter: Frontmatter {
                title: title.map(String::from),
                ..Default::default()
            },
            headings: vec![],
            first_paragraph: None,
            body: String::new(),
        }
    }

    #[test]
    fn builds_node_per_note_with_entry_paths() {
        let nodes = build_nodes(vec![note("README.md", None), note("docs/a.md", Some("A"))]);
        assert_eq!(nodes.len(), 2);
        assert_eq!(
            nodes[0].output_path,
            PathBuf::from("fragments/README/index.md")
        );
        assert_eq!(nodes[0].entry_dir, PathBuf::from("fragments/README"));
        assert_eq!(
            nodes[1].output_path,
            PathBuf::from("fragments/docs/a/index.md")
        );
        assert_eq!(nodes[0].title, "README");
        assert_eq!(nodes[1].title, "A");
    }
}
