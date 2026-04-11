use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::types::{KeyFile, ScanResult, WikiNode};

pub fn analyze(scan: &ScanResult) -> Vec<WikiNode> {
    let child_dirs: BTreeSet<PathBuf> = scan
        .files
        .iter()
        .filter(|f| f.components().count() > 1)
        .filter_map(|f| f.components().next().map(|c| PathBuf::from(c.as_os_str())))
        .collect();

    let dir_nodes: Vec<PathBuf> = child_dirs.into_iter().collect();

    let mut nodes = Vec::new();

    // index
    nodes.push(WikiNode {
        title: scan
            .root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Project".to_string()),
        output_path: PathBuf::from("index.md"),
        summary: String::new(),
        key_files: scan
            .files
            .iter()
            .filter(|f| f.components().count() == 1)
            .map(|f| KeyFile {
                path: f.clone(),
                description: String::new(),
            })
            .collect(),
        responsibilities: Vec::new(),
        related: Vec::new(),
        read_next: dir_nodes
            .iter()
            .map(|d| PathBuf::from(format!("directories/{}.md", d.display())))
            .collect(),
    });

    // ディレクトリごとのノード
    for dir in &dir_nodes {
        let scope_files: Vec<PathBuf> = scan
            .files
            .iter()
            .filter(|f| f.starts_with(dir))
            .cloned()
            .collect();

        let siblings: Vec<PathBuf> = dir_nodes
            .iter()
            .filter(|d| *d != dir)
            .map(|d| PathBuf::from(format!("directories/{}.md", d.display())))
            .collect();

        nodes.push(WikiNode {
            title: dir.to_string_lossy().to_string(),
            output_path: PathBuf::from(format!("directories/{}.md", dir.display())),
            summary: String::new(),
            key_files: scope_files
                .iter()
                .map(|f| KeyFile {
                    path: f.clone(),
                    description: String::new(),
                })
                .collect(),
            responsibilities: Vec::new(),
            related: siblings,
            read_next: Vec::new(),
        });
    }

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ScanResult;

    #[test]
    fn creates_index_and_directory_nodes() {
        let scan = ScanResult {
            root: PathBuf::from("/tmp/myproject"),
            files: vec![
                PathBuf::from("README.md"),
                PathBuf::from("src/main.rs"),
                PathBuf::from("src/lib.rs"),
                PathBuf::from("tests/test_main.rs"),
            ],
        };

        let nodes = analyze(&scan);

        assert!(
            nodes
                .iter()
                .any(|n| n.output_path == PathBuf::from("index.md"))
        );
        assert!(
            nodes
                .iter()
                .any(|n| n.output_path == PathBuf::from("directories/src.md"))
        );
        assert!(
            nodes
                .iter()
                .any(|n| n.output_path == PathBuf::from("directories/tests.md"))
        );
        assert_eq!(nodes.len(), 3); // index + src + tests
    }

    #[test]
    fn index_read_next_points_to_directories() {
        let scan = ScanResult {
            root: PathBuf::from("/tmp/proj"),
            files: vec![PathBuf::from("src/main.rs")],
        };

        let nodes = analyze(&scan);
        let index = nodes
            .iter()
            .find(|n| n.output_path == PathBuf::from("index.md"))
            .unwrap();
        assert!(
            index
                .read_next
                .contains(&PathBuf::from("directories/src.md"))
        );
    }
}
