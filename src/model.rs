use std::path::PathBuf;

use crate::notes::NoteData;

/// 取り込んだ 1 ノートに対応する内部モデル。
#[derive(Debug, Clone)]
pub struct Node {
    /// 出力先ルートからの相対パス（例: `notes/docs/foo.md`）
    pub output_path: PathBuf,
    pub title: String,
    pub note: NoteData,
    /// Phase 2 で計算する関連ノード（出力相対パス）
    pub related: Vec<PathBuf>,
    pub backlinks: Vec<PathBuf>,
}
