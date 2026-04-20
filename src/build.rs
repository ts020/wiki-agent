use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::extract::{extract_symbols, sort_symbols};
use crate::model::{Node, NodeKind, SYMBOL_NODE_LIMIT};
use crate::render::paths::{code_node_path, resolve_conflict};
use crate::scan::ScannedFile;

/// 走査済みファイル集合からコード由来ノードを組み立てる。
///
/// - 直接ファイルを含むディレクトリごとに 1 ノード生成する。
/// - ルート（relative parent が空）に置かれたファイルは `_root` ノードに集約。
/// - 各ノード内でシンボルを抽出し、閾値超過時は退避パスを付与する。
pub fn build_code_nodes(scanned: &[ScannedFile], target_root: &Path) -> Vec<Node> {
    let mut dirs: BTreeMap<PathBuf, Vec<ScannedFile>> = BTreeMap::new();
    for f in scanned {
        let parent = f
            .relative_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default();
        dirs.entry(parent).or_default().push(f.clone());
    }

    if dirs.is_empty() {
        dirs.insert(PathBuf::new(), vec![]);
    }

    let mut used: HashSet<PathBuf> = HashSet::new();
    let mut nodes = Vec::with_capacity(dirs.len());
    for (dir, mut files) in dirs {
        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        let title = node_title(&dir);
        let output = resolve_conflict(code_node_path(&dir), &mut used);

        let mut symbols = extract_symbols(&files, target_root);
        sort_symbols(&mut symbols);
        let overflow_path = if symbols.len() > SYMBOL_NODE_LIMIT {
            Some(overflow_output_path(&output))
        } else {
            None
        };

        let key_files: Vec<PathBuf> = files.into_iter().map(|f| f.relative_path).collect();

        nodes.push(Node {
            kind: NodeKind::CodeDerived,
            output_path: output,
            title,
            source_dir: dir,
            key_files,
            symbols,
            symbols_overflow_path: overflow_path,
        });
    }
    nodes
}

fn node_title(dir: &Path) -> String {
    if dir.as_os_str().is_empty() {
        "/".to_string()
    } else {
        dir.display().to_string()
    }
}

/// ノード出力パス `directories/<p>.md` に対応する overflow パス
/// `directories/<p>/_symbols.md` を算出する。
fn overflow_output_path(node_path: &Path) -> PathBuf {
    let stem = node_path
        .file_stem()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    let parent = node_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    parent.join(stem).join("_symbols.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sf(path: &str) -> ScannedFile {
        ScannedFile {
            relative_path: PathBuf::from(path),
            size: 0,
        }
    }

    #[test]
    fn groups_files_by_parent_directory() {
        let files = vec![
            sf("src/main.rs"),
            sf("src/lib.rs"),
            sf("src/scan/mod.rs"),
            sf("README.md"),
        ];
        let nodes = build_code_nodes(&files, Path::new("/nonexistent"));
        let titles: Vec<_> = nodes.iter().map(|n| n.title.clone()).collect();
        assert_eq!(titles, vec!["/", "src", "src/scan"]);
        assert_eq!(nodes[0].key_files, vec![PathBuf::from("README.md")]);
        assert_eq!(
            nodes[1].key_files,
            vec![PathBuf::from("src/lib.rs"), PathBuf::from("src/main.rs")]
        );
    }

    #[test]
    fn empty_scan_produces_root_node() {
        let nodes = build_code_nodes(&[], Path::new("/nonexistent"));
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].title, "/");
        assert_eq!(nodes[0].output_path, PathBuf::from("directories/_root.md"));
    }

    #[test]
    fn overflow_path_is_sibling_of_node() {
        assert_eq!(
            overflow_output_path(Path::new("directories/src.md")),
            PathBuf::from("directories/src/_symbols.md")
        );
        assert_eq!(
            overflow_output_path(Path::new("directories/_root.md")),
            PathBuf::from("directories/_root/_symbols.md")
        );
        assert_eq!(
            overflow_output_path(Path::new("directories/src/scan.md")),
            PathBuf::from("directories/src/scan/_symbols.md")
        );
    }
}
