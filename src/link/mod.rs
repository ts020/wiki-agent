pub mod resolver;
pub mod slug;
pub mod wikilink;

pub use resolver::{Resolver, UnresolvedLink};
pub use slug::slugify;
pub use wikilink::{WikiLink, find_all};

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::model::{Node, NodeKind};

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

/// すべてのノート由来ノードの本文中の wikilink を解決し、未解決一覧と
/// 集計済みのリンクグラフを返す。
/// ノート由来ノードのフロントマター `related` も同じ方式で解決し、
/// 解決できたものはグラフに edge として追加する。
pub fn resolve_all(nodes: &mut [Node]) -> (Vec<UnresolvedLink>, LinkGraph) {
    let resolver = Resolver::build(nodes);
    let known_paths: HashSet<PathBuf> = nodes.iter().map(|n| n.output_path.clone()).collect();
    let mut graph = LinkGraph::default();
    let mut unresolved = Vec::new();

    for n in nodes.iter_mut() {
        if !matches!(n.kind, NodeKind::NoteDerived) {
            continue;
        }
        let output = n.output_path.clone();

        // フロントマターの related 解決
        if let Some(note) = n.note.as_ref() {
            for entry in &note.frontmatter.related {
                let resolved = resolve_related_entry(entry, &output, &resolver, &known_paths);
                if let Some(target) = resolved {
                    graph.add_edge(&output, &target);
                }
            }
        }

        // 本文の wikilink 解決
        let Some(note) = n.note.as_mut() else {
            continue;
        };
        let (new_body, mut us, edges) = wikilink::resolve_in(&note.body, &output, &resolver);
        note.body = new_body;
        unresolved.append(&mut us);
        for edge in edges {
            graph.add_edge(&output, &edge);
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
