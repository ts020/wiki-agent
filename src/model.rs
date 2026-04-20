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
    /// 出力先ルートからの相対パス（例: `directories/src/scan.md`）
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
}

/// 1 ノードあたりのシンボル列挙上限（FR-07）。超過時は `_symbols.md` に退避する。
pub const SYMBOL_NODE_LIMIT: usize = 100;
