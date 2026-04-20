use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::link::LinkGraph;
use crate::model::{Node, NodeKind};
use crate::render::tags::TagIndex;

/// 全ノードに対して related / backlinks / read_next を計算して書き戻す。
pub fn compute_relations(nodes: &mut [Node], graph: &LinkGraph, tag_index: &TagIndex) {
    let paths_by_dir = index_code_nodes_by_dir(nodes);
    let tags_by_node = index_tags_by_node(nodes);

    // すべて先に読んでから mutate するため、インデックスを持つ
    let snapshot: Vec<(PathBuf, NodeKind, Vec<String>)> = nodes
        .iter()
        .map(|n| {
            let tags = n
                .note
                .as_ref()
                .map(|nd| nd.frontmatter.tags.clone())
                .unwrap_or_default();
            (n.output_path.clone(), n.kind.clone(), tags)
        })
        .collect();

    // いったん結果を集めてから書き戻す（借用制約のため）
    let mut results: Vec<(Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>)> =
        Vec::with_capacity(nodes.len());
    for (out_path, kind, tags) in &snapshot {
        let backlinks: Vec<PathBuf> = graph.backward_of(out_path);

        let related: Vec<PathBuf> = match kind {
            NodeKind::CodeDerived => {
                let mut set: BTreeSet<PathBuf> = BTreeSet::new();
                // forward/backward リンクがあれば混ぜる
                for e in graph.forward_of(out_path) {
                    set.insert(e);
                }
                for e in graph.backward_of(out_path) {
                    set.insert(e);
                }
                // パス隣接
                for p in path_siblings(out_path, &paths_by_dir) {
                    set.insert(p);
                }
                set.remove(out_path);
                set.into_iter().collect()
            }
            NodeKind::NoteDerived => {
                let mut set: BTreeSet<PathBuf> = BTreeSet::new();
                for e in graph.forward_of(out_path) {
                    set.insert(e);
                }
                set.remove(out_path);
                set.into_iter().collect()
            }
        };

        let read_next: Vec<PathBuf> = match kind {
            NodeKind::CodeDerived => path_read_next(out_path, &paths_by_dir)
                .into_iter()
                .filter(|p| p != out_path)
                .collect(),
            NodeKind::NoteDerived => tag_shared(out_path, tags, tag_index, &tags_by_node),
        };

        results.push((related, backlinks, read_next));
    }

    for (n, (related, backlinks, read_next)) in nodes.iter_mut().zip(results.into_iter()) {
        n.related = related;
        n.backlinks = backlinks;
        n.read_next = read_next;
    }
}

fn index_code_nodes_by_dir(nodes: &[Node]) -> BTreeMap<PathBuf, Vec<PathBuf>> {
    let mut by_parent: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for n in nodes {
        if !matches!(n.kind, NodeKind::CodeDerived) {
            continue;
        }
        let parent = n
            .output_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        by_parent
            .entry(parent)
            .or_default()
            .push(n.output_path.clone());
    }
    for v in by_parent.values_mut() {
        v.sort();
    }
    by_parent
}

fn index_tags_by_node(nodes: &[Node]) -> BTreeMap<PathBuf, BTreeSet<String>> {
    let mut out: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for n in nodes {
        if let Some(note) = &n.note {
            let tags: BTreeSet<String> = note.frontmatter.tags.iter().cloned().collect();
            if !tags.is_empty() {
                out.insert(n.output_path.clone(), tags);
            }
        }
    }
    out
}

fn path_siblings(out_path: &Path, paths_by_dir: &BTreeMap<PathBuf, Vec<PathBuf>>) -> Vec<PathBuf> {
    let parent = out_path.parent().map(Path::to_path_buf).unwrap_or_default();
    paths_by_dir
        .get(&parent)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p != out_path)
        .collect()
}

/// コード由来ノードの Read next: 子ディレクトリ候補 + 同階層
fn path_read_next(out_path: &Path, paths_by_dir: &BTreeMap<PathBuf, Vec<PathBuf>>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    // 子: 同名 stem のサブディレクトリ配下
    let stem = out_path.file_stem().map(|s| s.to_os_string());
    let parent = out_path.parent().map(Path::to_path_buf).unwrap_or_default();
    if let Some(stem) = stem {
        let child_dir = parent.join(&stem);
        if let Some(children) = paths_by_dir.get(&child_dir) {
            out.extend(children.iter().cloned());
        }
    }
    // 同階層の兄弟
    if let Some(siblings) = paths_by_dir.get(&parent) {
        for p in siblings {
            if p != out_path && !out.contains(p) {
                out.push(p.clone());
            }
        }
    }
    out
}

/// ノート由来ノードの Read next: タグ共通のノード（重複タグ数で降順）
fn tag_shared(
    out_path: &Path,
    tags: &[String],
    _tag_index: &TagIndex,
    tags_by_node: &BTreeMap<PathBuf, BTreeSet<String>>,
) -> Vec<PathBuf> {
    if tags.is_empty() {
        return Vec::new();
    }
    let self_tags: BTreeSet<&str> = tags.iter().map(String::as_str).collect();
    let mut scored: Vec<(usize, PathBuf)> = Vec::new();
    for (path, other_tags) in tags_by_node {
        if path == out_path {
            continue;
        }
        let overlap = other_tags
            .iter()
            .filter(|t| self_tags.contains(t.as_str()))
            .count();
        if overlap > 0 {
            scored.push((overlap, path.clone()));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, p)| p).collect()
}
