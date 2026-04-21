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
    /// ノード → ターゲットパスの射影を渡して任意の出力パス体系で解決できる。
    pub fn build_with<F: Fn(&Node) -> PathBuf>(nodes: &[Node], path_of: F) -> Self {
        let mut r = Self::default();
        for n in nodes {
            let target = path_of(n);
            if let Some(stem) = target.file_stem().and_then(|s| s.to_str()) {
                r.by_basename
                    .entry(stem.to_string())
                    .or_default()
                    .push(target.clone());
            }
            if let Some(note) = &n.note {
                for alias in &note.frontmatter.aliases {
                    r.by_alias
                        .entry(alias.clone())
                        .or_default()
                        .push(target.clone());
                }
                if let Some(title) = &note.frontmatter.title {
                    r.by_alias
                        .entry(title.clone())
                        .or_default()
                        .push(target.clone());
                }
            }
        }
        r
    }

    /// 既定: `output_path`（索引ページのパス）を解決ターゲットにする。
    pub fn build(nodes: &[Node]) -> Self {
        Self::build_with(nodes, |n| n.output_path.clone())
    }

    /// 原本コピー用: ノートは `content_path`、コードノードは `output_path`。
    pub fn build_for_content(nodes: &[Node]) -> Self {
        Self::build_with(nodes, |n| {
            n.content_path
                .clone()
                .unwrap_or_else(|| n.output_path.clone())
        })
    }

    /// 指定された名前を解決する。`from` は参照元ノードの出力相対パス。
    pub fn resolve(&self, name: &str, from: &Path) -> Option<PathBuf> {
        if let Some(list) = self.by_alias.get(name)
            && let Some(p) = pick_best(list, from)
        {
            return Some(p);
        }
        if let Some(list) = self.by_basename.get(name)
            && let Some(p) = pick_best(list, from)
        {
            return Some(p);
        }
        None
    }
}

fn pick_best(paths: &[PathBuf], from: &Path) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }
    if paths.len() == 1 {
        return Some(paths[0].clone());
    }
    let from_parent = from.parent().unwrap_or(Path::new(""));
    // 優先度 1: 同じディレクトリ
    for p in paths {
        if p.parent().unwrap_or(Path::new("")) == from_parent {
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
    /// 参照元ノードの出力相対パス
    pub source: PathBuf,
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Node, NodeKind};

    fn code(path: &str) -> Node {
        Node {
            kind: NodeKind::CodeDerived,
            output_path: PathBuf::from(path),
            title: "t".into(),
            source_dir: PathBuf::new(),
            key_files: vec![],
            symbols: vec![],
            symbols_overflow_path: None,
            note: None,
            content_path: None,
            related: vec![],
            backlinks: vec![],
            read_next: vec![],
        }
    }

    #[test]
    fn resolves_by_basename() {
        let nodes = vec![code("code-nodes/src.md"), code("note-index/foo.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("note-index/other.md")),
            Some(PathBuf::from("note-index/foo.md"))
        );
    }

    #[test]
    fn same_directory_wins_on_collision() {
        let nodes = vec![code("note-index/a/foo.md"), code("note-index/b/foo.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("note-index/a/bar.md")),
            Some(PathBuf::from("note-index/a/foo.md"))
        );
        assert_eq!(
            r.resolve("foo", Path::new("note-index/b/bar.md")),
            Some(PathBuf::from("note-index/b/foo.md"))
        );
    }

    #[test]
    fn shortest_path_then_alpha_wins() {
        let nodes = vec![
            code("note-index/deep/deep/foo.md"),
            code("note-index/foo.md"),
        ];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("note-index/other/here.md")),
            Some(PathBuf::from("note-index/foo.md"))
        );
    }

    #[test]
    fn returns_none_when_unknown() {
        let nodes = vec![code("note-index/foo.md")];
        let r = Resolver::build(&nodes);
        assert!(r.resolve("bar", Path::new("note-index/x.md")).is_none());
    }
}
