use std::path::PathBuf;

use crate::extract::LocatedSymbol;
use crate::notes::NoteData;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    CodeDerived,
    NoteDerived,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    /// 出力先ルートからの相対パス（例: `code-nodes/src/scan.md`, `note-index/docs/foo.md`）
    pub output_path: PathBuf,
    pub title: String,

    // --- コード由来ノード専用（ノート由来時はデフォルト） ---
    /// 走査対象ルートからの相対ディレクトリパス
    pub source_dir: PathBuf,
    /// 走査対象ルートからの相対ファイルパス
    pub key_files: Vec<PathBuf>,
    /// このノードに属するシンボル（ソート済み）
    pub symbols: Vec<LocatedSymbol>,
    /// シンボル数が閾値を超えた場合に退避先 `_symbols.md` の出力相対パス
    pub symbols_overflow_path: Option<PathBuf>,

    // --- ノート由来ノード専用（コード由来時は None） ---
    pub note: Option<NoteData>,
    /// ノート由来時のみ: 原本本文コピーの出力相対パス（`imported/<rel>`）
    pub content_path: Option<PathBuf>,

    // --- Phase 8 で設定される関連リンク群（いずれもこのノード出力パスへの相対にするのは render 層） ---
    pub related: Vec<PathBuf>,
    pub backlinks: Vec<PathBuf>,
    pub read_next: Vec<PathBuf>,
}

/// 1 ノードあたりのシンボル列挙上限（FR-07）。超過時は `_symbols.md` に退避する。
pub const SYMBOL_NODE_LIMIT: usize = 100;
