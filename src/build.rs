use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::model::{Node, NodeKind};
use crate::render::paths::{code_node_path, resolve_conflict};
use crate::scan::ScannedFile;

/// 走査済みファイル集合からコード由来ノードを組み立てる。
///
/// - 直接ファイルを含むディレクトリごとに 1 ノード生成する。
/// - ルート（relative parent が空）に置かれたファイルは `_root` ノードに集約。
/// - 出力パスは決定論順（BTreeMap）。衝突は `-N` で解消。
pub fn build_code_nodes(scanned: &[ScannedFile]) -> Vec<Node> {
    let mut dirs: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for f in scanned {
        let parent = f
            .relative_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default();
        dirs.entry(parent)
            .or_default()
            .push(f.relative_path.clone());
    }

    if dirs.is_empty() {
        dirs.insert(PathBuf::new(), vec![]);
    }

    let mut used: HashSet<PathBuf> = HashSet::new();
    let mut nodes = Vec::with_capacity(dirs.len());
    for (dir, mut files) in dirs {
        files.sort();
        let title = node_title(&dir);
        let output = resolve_conflict(code_node_path(&dir), &mut used);
        nodes.push(Node {
            kind: NodeKind::CodeDerived,
            output_path: output,
            title,
            source_dir: dir,
            key_files: files,
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
        let nodes = build_code_nodes(&files);
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
        let nodes = build_code_nodes(&[]);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].title, "/");
        assert_eq!(nodes[0].output_path, PathBuf::from("directories/_root.md"));
    }
}
