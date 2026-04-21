use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::link::LinkGraph;
use crate::model::{Node, iter_pages};
use crate::render::tags::TagIndex;

const RELATED_MAX: usize = 3;
const RELATED_MIN_SHARED_TAGS: usize = 2;

type NodeSnapshot = (PathBuf, Vec<String>, Vec<String>, Vec<PathBuf>);
type ComputeResult = (Vec<PathBuf>, BTreeMap<PathBuf, Vec<PathBuf>>);

/// 全ノートに対して related / backlinks を計算して書き戻す（FR-10, FR-11）。
/// Backlinks は断片解像度（各ページ単位）、Related は入口ページ単位。
pub fn compute_relations(nodes: &mut [Node], graph: &LinkGraph, _tag_index: &TagIndex) {
    let tags_by_node: BTreeMap<PathBuf, BTreeSet<String>> = nodes
        .iter()
        .map(|n| {
            (
                n.output_path.clone(),
                n.note.frontmatter.tags.iter().cloned().collect(),
            )
        })
        .collect();

    let known_paths: BTreeSet<PathBuf> = nodes.iter().map(|n| n.output_path.clone()).collect();

    let snapshot: Vec<NodeSnapshot> = nodes
        .iter()
        .map(|n| {
            let page_paths: Vec<PathBuf> =
                iter_pages(n).into_iter().map(|p| p.output_path).collect();
            (
                n.output_path.clone(),
                n.note.frontmatter.related.clone(),
                n.note.frontmatter.tags.clone(),
                page_paths,
            )
        })
        .collect();

    // (related, backlinks-map)
    let mut results: Vec<ComputeResult> = Vec::with_capacity(nodes.len());
    for (entry, fm_related, tags, pages) in &snapshot {
        let related = compute_related(entry, fm_related, tags, graph, &tags_by_node, &known_paths);
        let mut backlinks: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
        for page in pages {
            let refs: Vec<PathBuf> = graph.backward_of(page);
            if !refs.is_empty() {
                backlinks.insert(page.clone(), refs);
            }
        }
        results.push((related, backlinks));
    }

    for (n, (related, backlinks)) in nodes.iter_mut().zip(results.into_iter()) {
        n.related = related;
        n.backlinks = backlinks;
    }
}

/// FR-11: フロントマター related を最優先、共通タグ 2 つ以上のノートで補完（最大 3 件）。
fn compute_related(
    self_path: &Path,
    fm_related: &[String],
    tags: &[String],
    graph: &LinkGraph,
    tags_by_node: &BTreeMap<PathBuf, BTreeSet<String>>,
    known_paths: &BTreeSet<PathBuf>,
) -> Vec<PathBuf> {
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut out: Vec<PathBuf> = Vec::new();

    let forward = graph.forward_of(self_path);
    let forward_set: BTreeSet<PathBuf> = forward.iter().cloned().collect();
    for entry in fm_related {
        if let Some(path) = resolve_related_path(entry, &forward_set, known_paths)
            && path != self_path
            && !seen.contains(&path)
        {
            seen.insert(path.clone());
            out.push(path);
        }
    }

    if !tags.is_empty() {
        let self_tags: BTreeSet<&str> = tags.iter().map(String::as_str).collect();
        let mut scored: Vec<(usize, PathBuf)> = Vec::new();
        for (path, other_tags) in tags_by_node {
            if path == self_path || seen.contains(path) {
                continue;
            }
            let overlap = other_tags
                .iter()
                .filter(|t| self_tags.contains(t.as_str()))
                .count();
            if overlap >= RELATED_MIN_SHARED_TAGS {
                scored.push((overlap, path.clone()));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        for (_, path) in scored.into_iter().take(RELATED_MAX) {
            seen.insert(path.clone());
            out.push(path);
        }
    }

    out
}

/// related エントリ（wikilink ターゲット or 出力相対パス）を既知のパスに解決する。
fn resolve_related_path(
    entry: &str,
    forward_set: &BTreeSet<PathBuf>,
    known_paths: &BTreeSet<PathBuf>,
) -> Option<PathBuf> {
    let as_path = PathBuf::from(entry);
    if known_paths.contains(&as_path) {
        return Some(as_path);
    }
    for p in forward_set {
        if p.file_stem().and_then(|s| s.to_str()) == Some(entry) {
            return Some(p.clone());
        }
    }
    None
}
