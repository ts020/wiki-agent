use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::types::{KeyFile, ScanResult, WikiNode};

pub fn analyze(scan: &ScanResult) -> Vec<WikiNode> {
    let mut all_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for f in &scan.files {
        if let Some(parent) = f.parent() {
            let mut p = parent;
            while p != Path::new("") {
                all_dirs.insert(p.to_path_buf());
                match p.parent() {
                    Some(pp) => p = pp,
                    None => break,
                }
            }
        }
    }

    let mut nodes = Vec::new();

    // index
    let root_file_paths: Vec<&PathBuf> = scan
        .files
        .iter()
        .filter(|f| f.parent() == Some(Path::new("")) || f.components().count() == 1)
        .collect();

    let mut root_read_next: Vec<PathBuf> = all_dirs
        .iter()
        .filter(|d| d.components().count() == 1)
        .map(|d| dir_output_path(d))
        .collect();
    for f in &root_file_paths {
        root_read_next.push(file_output_path(f));
    }

    nodes.push(WikiNode {
        title: scan
            .root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Project".to_string()),
        output_path: PathBuf::from("index.md"),
        summary: String::new(),
        key_files: root_file_paths
            .iter()
            .map(|f| KeyFile {
                path: (*f).clone(),
                description: String::new(),
            })
            .collect(),
        responsibilities: Vec::new(),
        related: Vec::new(),
        read_next: root_read_next,
    });

    // ディレクトリノード
    for dir in &all_dirs {
        let direct_files: Vec<&PathBuf> = scan
            .files
            .iter()
            .filter(|f| f.parent() == Some(dir.as_path()))
            .collect();

        let mut read_next: Vec<PathBuf> = all_dirs
            .iter()
            .filter(|d| d.parent() == Some(dir.as_path()))
            .map(|d| dir_output_path(d))
            .collect();
        for f in &direct_files {
            read_next.push(file_output_path(f));
        }

        let siblings: Vec<PathBuf> = all_dirs
            .iter()
            .filter(|d| d.parent() == dir.parent() && *d != dir)
            .map(|d| dir_output_path(d))
            .collect();

        nodes.push(WikiNode {
            title: dir.to_string_lossy().to_string(),
            output_path: dir_output_path(dir),
            summary: String::new(),
            key_files: direct_files
                .iter()
                .map(|f| KeyFile {
                    path: (*f).clone(),
                    description: String::new(),
                })
                .collect(),
            responsibilities: Vec::new(),
            related: siblings,
            read_next,
        });
    }

    // ファイルノード（末端）
    for f in &scan.files {
        let siblings: Vec<PathBuf> = scan
            .files
            .iter()
            .filter(|s| s.parent() == f.parent() && *s != f)
            .map(|s| file_output_path(s))
            .collect();

        nodes.push(WikiNode {
            title: f
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            output_path: file_output_path(f),
            summary: String::new(),
            key_files: vec![KeyFile {
                path: f.clone(),
                description: String::new(),
            }],
            responsibilities: Vec::new(),
            related: siblings,
            read_next: Vec::new(),
        });
    }

    nodes
}

fn dir_output_path(dir: &Path) -> PathBuf {
    PathBuf::from(format!("directories/{}.md", dir.display()))
}

fn file_output_path(file: &Path) -> PathBuf {
    let without_ext = file.with_extension("");
    PathBuf::from(format!("files/{}.md", without_ext.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ScanResult;

    #[test]
    fn creates_file_nodes() {
        let scan = ScanResult {
            root: PathBuf::from("/tmp/proj"),
            files: vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")],
        };

        let nodes = analyze(&scan);
        assert!(
            nodes
                .iter()
                .any(|n| n.output_path == PathBuf::from("files/src/main.md"))
        );
        assert!(
            nodes
                .iter()
                .any(|n| n.output_path == PathBuf::from("files/src/lib.md"))
        );
    }

    #[test]
    fn directory_read_next_includes_files() {
        let scan = ScanResult {
            root: PathBuf::from("/tmp/proj"),
            files: vec![
                PathBuf::from("src/main.rs"),
                PathBuf::from("src/handlers/auth.rs"),
            ],
        };

        let nodes = analyze(&scan);
        let src = nodes
            .iter()
            .find(|n| n.output_path == PathBuf::from("directories/src.md"))
            .unwrap();
        assert!(
            src.read_next
                .contains(&PathBuf::from("directories/src/handlers.md"))
        );
        assert!(src.read_next.contains(&PathBuf::from("files/src/main.md")));
    }

    #[test]
    fn file_nodes_have_siblings() {
        let scan = ScanResult {
            root: PathBuf::from("/tmp/proj"),
            files: vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")],
        };

        let nodes = analyze(&scan);
        let main_node = nodes
            .iter()
            .find(|n| n.output_path == PathBuf::from("files/src/main.md"))
            .unwrap();
        assert!(
            main_node
                .related
                .contains(&PathBuf::from("files/src/lib.md"))
        );
    }

    #[test]
    fn file_nodes_are_leaf() {
        let scan = ScanResult {
            root: PathBuf::from("/tmp/proj"),
            files: vec![PathBuf::from("src/main.rs")],
        };

        let nodes = analyze(&scan);
        let main_node = nodes
            .iter()
            .find(|n| n.output_path == PathBuf::from("files/src/main.md"))
            .unwrap();
        assert!(main_node.read_next.is_empty());
    }
}
