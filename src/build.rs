use std::collections::HashSet;
use std::path::PathBuf;

use crate::model::Node;
use crate::notes::NoteData;
use crate::render::paths::{note_path, resolve_conflict};

/// 取り込んだノートから内部モデルを組み立てる。出力パスは `notes/<rel>`。
pub fn build_nodes(notes: Vec<NoteData>) -> Vec<Node> {
    let mut used: HashSet<PathBuf> = HashSet::new();
    let mut out = Vec::with_capacity(notes.len());
    for data in notes {
        let output = resolve_conflict(note_path(&data.source_file), &mut used);
        let title = derive_title(&data);
        out.push(Node {
            output_path: output,
            title,
            note: data,
            related: Vec::new(),
            backlinks: Vec::new(),
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
    fn builds_node_per_note() {
        let nodes = build_nodes(vec![note("README.md", None), note("docs/a.md", Some("A"))]);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].output_path, PathBuf::from("notes/README.md"));
        assert_eq!(nodes[1].output_path, PathBuf::from("notes/docs/a.md"));
        assert_eq!(nodes[0].title, "README");
        assert_eq!(nodes[1].title, "A");
    }
}
