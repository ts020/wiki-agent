use std::fs;

use repo_wiki::build::build_code_nodes;
use repo_wiki::render::write_wiki;
use repo_wiki::scan::{ScanConfig, scan};
use tempfile::TempDir;

#[test]
fn generates_index_and_directory_nodes() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("src/scan")).unwrap();
    fs::write(target.join("README.md"), "hello").unwrap();
    fs::write(target.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(target.join("src/scan/mod.rs"), "pub fn scan() {}").unwrap();

    let output = tmp.path().join("out");

    let files = scan(&ScanConfig {
        root: target.clone(),
        extra_excluded: Vec::new(),
    });
    let nodes = build_code_nodes(&files);
    write_wiki(&output, "project", &nodes).unwrap();

    let index = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(
        index.contains("# project"),
        "index must contain project title"
    );
    assert!(index.contains("## Directories"));
    assert!(index.contains("directories/_root.md"));
    assert!(index.contains("directories/src.md"));
    assert!(index.contains("directories/src/scan.md"));

    let root = fs::read_to_string(output.join("directories/_root.md")).unwrap();
    assert!(root.contains("README.md"));

    let src = fs::read_to_string(output.join("directories/src.md")).unwrap();
    assert!(src.contains("src/main.rs"));

    let scan_node = fs::read_to_string(output.join("directories/src/scan.md")).unwrap();
    assert!(scan_node.contains("src/scan/mod.rs"));
}

#[test]
fn output_directory_inside_target_is_excluded_from_scan() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("code.rs"), "fn main() {}").unwrap();

    // 既存の出力ディレクトリをターゲット内に配置し、前回生成物を置いておく
    let output = target.join("repo-wiki");
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("index.md"), "old").unwrap();

    let output_abs = std::path::absolute(&output).unwrap();
    let files = scan(&ScanConfig {
        root: target.clone(),
        extra_excluded: vec![output_abs],
    });

    let paths: Vec<_> = files.iter().map(|f| f.relative_path.clone()).collect();
    assert!(paths.contains(&std::path::PathBuf::from("code.rs")));
    assert!(!paths.iter().any(|p| p.starts_with("repo-wiki")));
}

#[test]
fn rerun_clears_previous_output() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.txt"), "x").unwrap();

    let output = tmp.path().join("out");
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("stale.md"), "old").unwrap();

    let files = scan(&ScanConfig {
        root: target.clone(),
        extra_excluded: Vec::new(),
    });
    let nodes = build_code_nodes(&files);
    write_wiki(&output, "project", &nodes).unwrap();

    assert!(!output.join("stale.md").exists());
    assert!(output.join("index.md").exists());
}
