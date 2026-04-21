use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::fragment::FragmentTree;
use crate::notes::NoteData;

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
    /// 各ページ（入口・殻・断片）への被参照リスト。F-4 で埋める。
    /// key: ページの出力相対パス / value: 参照元ページ相対パス（安定ソート済み）
    pub backlinks: BTreeMap<PathBuf, Vec<PathBuf>>,
}
