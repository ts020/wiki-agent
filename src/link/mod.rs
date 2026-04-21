pub mod resolver;
pub mod slug;
pub mod wikilink;

pub use resolver::{Resolver, UnresolvedLink};
pub use slug::slugify;
pub use wikilink::{WikiLink, find_all};

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::model::Node;

/// ノード間リンクの双方向グラフ。
#[derive(Debug, Default)]
pub struct LinkGraph {
    pub forward: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    pub backward: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl LinkGraph {
    pub fn add_edge(&mut self, from: &Path, to: &Path) {
        if from == to {
            return;
        }
        self.forward
            .entry(from.to_path_buf())
            .or_default()
            .insert(to.to_path_buf());
        self.backward
            .entry(to.to_path_buf())
            .or_default()
            .insert(from.to_path_buf());
    }

    pub fn forward_of(&self, path: &Path) -> Vec<PathBuf> {
        self.forward
            .get(path)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
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
pub fn resolve_all(nodes: &[Node]) -> (Vec<UnresolvedLink>, LinkGraph) {
    let resolver = Resolver::build(nodes);
    let known_paths: HashSet<PathBuf> = nodes.iter().map(|n| n.output_path.clone()).collect();
    let mut graph = LinkGraph::default();
    let mut unresolved = Vec::new();

    for n in nodes.iter() {
        let output = &n.output_path;

        // フロントマターの related
        for entry in &n.note.frontmatter.related {
            if let Some(target) = resolve_related_entry(entry, output, &resolver, &known_paths) {
                graph.add_edge(output, &target);
            }
        }

        // 本文の wikilink
        let (_rewritten, mut us, edges) =
            wikilink::resolve_in(&n.note.body, output, output, &resolver);
        unresolved.append(&mut us);
        for edge in edges {
            graph.add_edge(output, &edge);
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
