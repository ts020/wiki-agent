pub mod resolver;
pub mod slug;
pub mod wikilink;

pub use resolver::{Resolver, UnresolvedLink};
pub use slug::slugify;
pub use wikilink::{WikiLink, find_all};

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::model::{Node, iter_pages};

/// ページ間リンクの双方向グラフ。
/// - forward: 参照元ページ → 参照先ページ群。**本文出現順**を保持（FR-09）。
/// - backward: 参照先ページ → 参照元ページ群。**出力パス昇順**で安定（FR-10）。
#[derive(Debug, Default)]
pub struct LinkGraph {
    pub forward: BTreeMap<PathBuf, Vec<PathBuf>>,
    pub backward: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl LinkGraph {
    pub fn add_edge(&mut self, from: &Path, to: &Path) {
        if from == to {
            return;
        }
        let list = self.forward.entry(from.to_path_buf()).or_default();
        if !list.iter().any(|p| p == to) {
            list.push(to.to_path_buf());
        }
        self.backward
            .entry(to.to_path_buf())
            .or_default()
            .insert(from.to_path_buf());
    }

    pub fn forward_of(&self, path: &Path) -> Vec<PathBuf> {
        self.forward.get(path).cloned().unwrap_or_default()
    }

    pub fn backward_of(&self, path: &Path) -> Vec<PathBuf> {
        self.backward
            .get(path)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// すべてのノートの本文と `related` フィールドを解析して
/// 未解決一覧とリンクグラフを作る。本文は mutate しない。
///
/// F-4 以降: LinkGraph の頂点はページ（入口・殻・断片・子断片）単位。
/// `[[Foo]]` は Foo の入口ページへ、`[[Foo#h2]]` は該当 h2 断片／殻へ張られる。
pub fn resolve_all(nodes: &[Node]) -> (Vec<UnresolvedLink>, LinkGraph) {
    let resolver = Resolver::build(nodes);
    let known_entries: HashSet<PathBuf> = nodes.iter().map(|n| n.output_path.clone()).collect();
    let mut graph = LinkGraph::default();
    let mut unresolved = Vec::new();

    for n in nodes.iter() {
        let entry = &n.output_path;

        // フロントマターの related は入口 → 入口のエッジ
        for entry_name in &n.note.frontmatter.related {
            if let Some(target) =
                resolve_related_entry(entry_name, entry, &resolver, &known_entries)
            {
                graph.add_edge(entry, &target);
            }
        }

        // 各ページの本文から wikilink を抽出
        for page in iter_pages(n) {
            let (_rewritten, mut us, edges) =
                wikilink::resolve_in(&page.raw_body, &page.output_path, entry, &resolver);
            unresolved.append(&mut us);
            for edge in edges {
                graph.add_edge(&page.output_path, &edge);
            }
        }
    }

    unresolved.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then_with(|| a.target.cmp(&b.target))
    });
    (unresolved, graph)
}

fn resolve_related_entry(
    entry: &str,
    from: &Path,
    resolver: &Resolver,
    known_paths: &HashSet<PathBuf>,
) -> Option<PathBuf> {
    let as_path = PathBuf::from(entry);
    if known_paths.contains(&as_path) {
        return Some(as_path);
    }
    // basename (.md 除去) としても検索
    let stem = as_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(entry);
    resolver.resolve(stem, from)
}
