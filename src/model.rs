use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    CodeDerived,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    /// 出力先ルートからの相対パス（例: `directories/src/scan.md`）
    pub output_path: PathBuf,
    pub title: String,
    /// 走査対象ルートからの相対ディレクトリパス（コード由来ノードの場合）
    pub source_dir: PathBuf,
    /// 走査対象ルートからの相対ファイルパス
    pub key_files: Vec<PathBuf>,
}
