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
            if let Some(stem) = target.file_stem().and_then(|s| s.to_str()) {
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
    use crate::notes::{Frontmatter, NoteData};

    fn node(path: &str) -> Node {
        Node {
            output_path: PathBuf::from(path),
            title: "t".into(),
            note: NoteData {
                source_file: PathBuf::from(path),
                frontmatter: Frontmatter::default(),
                headings: vec![],
                first_paragraph: None,
                body: String::new(),
            },
            related: vec![],
            backlinks: vec![],
        }
    }

    #[test]
    fn resolves_by_basename() {
        let nodes = vec![node("notes/foo.md"), node("notes/docs/bar.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("notes/other.md")),
            Some(PathBuf::from("notes/foo.md"))
        );
    }

    #[test]
    fn same_directory_wins_on_collision() {
        let nodes = vec![node("notes/a/foo.md"), node("notes/b/foo.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("notes/a/bar.md")),
            Some(PathBuf::from("notes/a/foo.md"))
        );
    }

    #[test]
    fn shortest_path_then_alpha_wins() {
        let nodes = vec![node("notes/deep/deep/foo.md"), node("notes/foo.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("notes/other/here.md")),
            Some(PathBuf::from("notes/foo.md"))
        );
    }

    #[test]
    fn returns_none_when_unknown() {
        let nodes = vec![node("notes/foo.md")];
        let r = Resolver::build(&nodes);
        assert!(r.resolve("bar", Path::new("notes/x.md")).is_none());
    }
}
