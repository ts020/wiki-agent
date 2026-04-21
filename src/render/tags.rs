use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::PathBuf;

use super::paths::relative_link;
use crate::model::Node;

/// 全ノードのタグを集計した索引。
#[derive(Debug, Default)]
pub struct TagIndex {
    /// 正規化済みタグ名 → ノード出力パス一覧（ソート済み・重複なし）
    pub entries: BTreeMap<String, Vec<PathBuf>>,
}

pub fn build_tag_index(nodes: &[Node]) -> TagIndex {
    let mut map: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();
    for n in nodes {
        for tag in &n.note.frontmatter.tags {
            let norm: &str = tag.trim();
            if norm.is_empty() {
                continue;
            }
            for prefix in tag_prefixes(norm) {
                map.entry(prefix).or_default().insert(n.output_path.clone());
            }
        }
    }
    TagIndex {
        entries: map
            .into_iter()
            .map(|(k, set)| (k, set.into_iter().collect()))
            .collect(),
    }
}

/// `auth/session/x` → `["auth", "auth/session", "auth/session/x"]`
fn tag_prefixes(tag: &str) -> Vec<String> {
    let parts: Vec<&str> = tag.split('/').filter(|s| !s.is_empty()).collect();
    (1..=parts.len()).map(|n| parts[..n].join("/")).collect()
}

/// 正規化済みタグ名から出力パスを算出する。
/// `auth` → `tags/auth.md`
/// `auth/session` → `tags/auth/session.md`
pub fn tag_page_path(tag: &str) -> PathBuf {
    let parts: Vec<&str> = tag.split('/').filter(|s| !s.is_empty()).collect();
    let mut out = PathBuf::from("tags");
    let (last, rest) = parts
        .split_last()
        .expect("tag must have at least one segment");
    for seg in rest {
        out.push(seg);
    }
    out.push(format!("{last}.md"));
    out
}

/// `tags/index.md` の本文を生成する。全タグの入口ページ。
pub fn render_tag_index_page(tag_index: &TagIndex) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Tags");
    s.push('\n');
    if tag_index.entries.is_empty() {
        let _ = writeln!(&mut s, "_(no tags)_");
        return s;
    }
    let from = PathBuf::from("tags/index.md");
    for (tag, paths) in &tag_index.entries {
        let page = tag_page_path(tag);
        let link = relative_link(&from, &page);
        let _ = writeln!(&mut s, "- [`{tag}`]({link}) ({} 件)", paths.len());
    }
    s
}

pub fn render_tag_page(tag: &str, node_paths: &[PathBuf], nodes: &[Node]) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Tag: `{tag}`");
    s.push('\n');
    if node_paths.is_empty() {
        let _ = writeln!(&mut s, "_(no entries)_");
        return s;
    }
    let from = tag_page_path(tag);
    for p in node_paths {
        let title = nodes
            .iter()
            .find(|n| &n.output_path == p)
            .map(|n| n.title.clone())
            .unwrap_or_else(|| p.display().to_string());
        let link = relative_link(&from, p);
        let _ = writeln!(&mut s, "- [{title}]({link})");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_prefixes_flat_and_nested() {
        assert_eq!(tag_prefixes("auth"), vec!["auth"]);
        assert_eq!(tag_prefixes("auth/session"), vec!["auth", "auth/session"]);
        assert_eq!(tag_prefixes("a/b/c"), vec!["a", "a/b", "a/b/c"]);
    }

    #[test]
    fn tag_page_path_flat_and_nested() {
        assert_eq!(tag_page_path("auth"), PathBuf::from("tags/auth.md"));
        assert_eq!(
            tag_page_path("auth/session"),
            PathBuf::from("tags/auth/session.md")
        );
    }
}
