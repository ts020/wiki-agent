use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::Node;

/// wikilink 解決のためのインデックス。すべて所有データなので借用問題を避ける。
#[derive(Debug, Default)]
pub struct Resolver {
    by_basename: BTreeMap<String, Vec<PathBuf>>,
    by_alias: BTreeMap<String, Vec<PathBuf>>,
}

impl Resolver {
    pub fn build(nodes: &[Node]) -> Self {
        let mut r = Self::default();
        for n in nodes {
            let target = n.output_path.clone();
            // 出力パスは `fragments/<rel>/index.md` のため、ノート basename は
            // 元ファイル側の file_stem から取る（入口ディレクトリ名と同じ）。
            if let Some(stem) = n.note.source_file.file_stem().and_then(|s| s.to_str()) {
                r.by_basename
                    .entry(stem.to_string())
                    .or_default()
                    .push(target.clone());
            }
            for alias in &n.note.frontmatter.aliases {
                r.by_alias
                    .entry(alias.clone())
                    .or_default()
                    .push(target.clone());
            }
            if let Some(title) = &n.note.frontmatter.title {
                r.by_alias
                    .entry(title.clone())
                    .or_default()
                    .push(target.clone());
            }
        }
        r
    }

    /// 指定された名前を解決する。`from_entry` は参照元ノードの入口ページの出力相対パス
    /// （`fragments/<rel>/index.md`）。同一ディレクトリ優先の判定に使う。
    pub fn resolve(&self, name: &str, from_entry: &Path) -> Option<PathBuf> {
        if let Some(list) = self.by_alias.get(name)
            && let Some(p) = pick_best(list, from_entry)
        {
            return Some(p);
        }
        if let Some(list) = self.by_basename.get(name)
            && let Some(p) = pick_best(list, from_entry)
        {
            return Some(p);
        }
        None
    }
}

/// `fragments/<rel>/index.md` 型のパスから、「ノートが置かれた元ディレクトリ」に
/// 相当する親（入口ディレクトリの親）を取り出す。
fn source_dir_of(entry_path: &Path) -> &Path {
    entry_path
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new(""))
}

fn pick_best(paths: &[PathBuf], from_entry: &Path) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }
    if paths.len() == 1 {
        return Some(paths[0].clone());
    }
    let from_dir = source_dir_of(from_entry);
    // 優先度 1: 同じ元ディレクトリ
    for p in paths {
        if source_dir_of(p) == from_dir {
            return Some(p.clone());
        }
    }
    // 優先度 2: 最短パス → アルファベット順
    let mut sorted = paths.to_vec();
    sorted.sort_by(|a, b| {
        a.components()
            .count()
            .cmp(&b.components().count())
            .then_with(|| a.cmp(b))
    });
    sorted.into_iter().next()
}

#[derive(Debug, Clone)]
pub struct UnresolvedLink {
    /// 参照元ページの出力相対パス（F-3 以降で断片単位）
    pub source: PathBuf,
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::{Frontmatter, NoteData};

    fn node(src: &str) -> Node {
        let source = PathBuf::from(src);
        let entry = crate::render::paths::entry_index_path(&source);
        let entry_dir = entry.parent().unwrap().to_path_buf();
        Node {
            output_path: entry,
            entry_dir,
            title: "t".into(),
            note: NoteData {
                source_file: source,
                frontmatter: Frontmatter::default(),
                headings: vec![],
                first_paragraph: None,
                body: String::new(),
            },
            fragments: crate::fragment::FragmentTree::default(),
            related: vec![],
            backlinks: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn resolves_by_basename() {
        let nodes = vec![node("foo.md"), node("docs/bar.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("fragments/other/index.md")),
            Some(PathBuf::from("fragments/foo/index.md"))
        );
    }

    #[test]
    fn same_directory_wins_on_collision() {
        let nodes = vec![node("a/foo.md"), node("b/foo.md")];
        let r = Resolver::build(&nodes);
        // 参照元 `a/bar.md` の入口は fragments/a/bar/index.md
        assert_eq!(
            r.resolve("foo", Path::new("fragments/a/bar/index.md")),
            Some(PathBuf::from("fragments/a/foo/index.md"))
        );
    }

    #[test]
    fn shortest_path_then_alpha_wins() {
        let nodes = vec![node("deep/deep/foo.md"), node("foo.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("fragments/other/here/index.md")),
            Some(PathBuf::from("fragments/foo/index.md"))
        );
    }

    #[test]
    fn returns_none_when_unknown() {
        let nodes = vec![node("foo.md")];
        let r = Resolver::build(&nodes);
        assert!(
            r.resolve("bar", Path::new("fragments/x/index.md"))
                .is_none()
        );
    }
}
