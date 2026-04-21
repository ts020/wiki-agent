use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::fragment::{Fragment, FragmentTree};
use crate::notes::NoteData;
use crate::render::paths::{fragment_leaf_path, h3_leaf_path, shell_index_path};

/// 取り込んだ 1 ノートに対応する内部モデル。
///
/// 1 ノートは「入口ページ」＋「断片ページ群（あれば）」から成る。
/// `output_path` は入口ページ、`entry_dir` はその親ディレクトリを指し、
/// 断片ページの出力パスは `entry_dir/<h2-slug>.md` のように組み立てる。
#[derive(Debug, Clone)]
pub struct Node {
    /// 出力ルートからの相対パス。入口ページ `fragments/<rel>/index.md`
    pub output_path: PathBuf,
    /// 入口ページの親ディレクトリ `fragments/<rel>/`
    pub entry_dir: PathBuf,
    pub title: String,
    pub note: NoteData,
    pub fragments: FragmentTree,
    /// 入口ページに付与する Related（ノート単位、FR-11）
    pub related: Vec<PathBuf>,
    /// 各ページ（入口・殻・断片）への被参照リスト。断片解像度。
    /// key: ページの出力相対パス / value: 参照元ページ相対パス（安定ソート済み）
    pub backlinks: BTreeMap<PathBuf, Vec<PathBuf>>,
}

/// 1 ノートに属する 1 つのページ（入口・殻・h2 断片・h3 子断片）。
#[derive(Debug, Clone)]
pub struct PageRef {
    pub output_path: PathBuf,
    pub title: String,
    pub raw_body: String,
    pub kind: PageKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageKind {
    Entry,
    H2Leaf,
    Shell,
    H3Leaf,
}

/// ノートが持つ全ページを出現順に列挙する。
/// 入口 → 断片 / 殻 → 子断片 の順。
pub fn iter_pages(node: &Node) -> Vec<PageRef> {
    let mut pages = Vec::new();
    pages.push(PageRef {
        output_path: node.output_path.clone(),
        title: node.title.clone(),
        raw_body: node.fragments.preface.clone(),
        kind: PageKind::Entry,
    });
    for frag in &node.fragments.fragments {
        match frag {
            Fragment::H2 {
                slug,
                heading,
                body,
            } => {
                pages.push(PageRef {
                    output_path: fragment_leaf_path(&node.entry_dir, slug),
                    title: heading.clone(),
                    raw_body: body.clone(),
                    kind: PageKind::H2Leaf,
                });
            }
            Fragment::Shell {
                slug,
                heading,
                preface,
                children,
            } => {
                pages.push(PageRef {
                    output_path: shell_index_path(&node.entry_dir, slug),
                    title: heading.clone(),
                    raw_body: preface.clone(),
                    kind: PageKind::Shell,
                });
                for child in children {
                    pages.push(PageRef {
                        output_path: h3_leaf_path(&node.entry_dir, slug, &child.slug),
                        title: child.heading.clone(),
                        raw_body: child.body.clone(),
                        kind: PageKind::H3Leaf,
                    });
                }
            }
        }
    }
    pages
}
