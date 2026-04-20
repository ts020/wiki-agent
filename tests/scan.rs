use std::fs;
use std::path::PathBuf;

use repo_wiki::scan::{ScanConfig, scan};
use tempfile::TempDir;

fn rel_names(files: &[repo_wiki::scan::ScannedFile]) -> Vec<PathBuf> {
    files.iter().map(|f| f.relative_path.clone()).collect()
}

#[test]
fn excludes_fixed_directories() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join("keep.txt"), "ok").unwrap();
    for excluded in [".git", "node_modules", "dist", "build", "target"] {
        let dir = root.join(excluded);
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("inside.txt"), "ignore").unwrap();
    }

    let files = scan(&ScanConfig {
        root: root.to_path_buf(),
    });
    assert_eq!(rel_names(&files), vec![PathBuf::from("keep.txt")]);
}

#[test]
fn excludes_hidden_dirs_except_wiki() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::create_dir(root.join(".config")).unwrap();
    fs::write(root.join(".config/x.txt"), "hidden").unwrap();

    fs::create_dir(root.join(".wiki")).unwrap();
    fs::write(root.join(".wiki/y.txt"), "allow").unwrap();

    let files = scan(&ScanConfig {
        root: root.to_path_buf(),
    });
    let paths = rel_names(&files);
    assert!(paths.contains(&PathBuf::from(".wiki/y.txt")));
    assert!(!paths.iter().any(|p| p.starts_with(".config")));
}

#[test]
fn skips_files_larger_than_1_mib() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let big = vec![b'a'; 1024 * 1024 + 1];
    fs::write(root.join("big.txt"), &big).unwrap();
    fs::write(root.join("small.txt"), "ok").unwrap();

    let files = scan(&ScanConfig {
        root: root.to_path_buf(),
    });
    let paths = rel_names(&files);
    assert!(paths.contains(&PathBuf::from("small.txt")));
    assert!(!paths.contains(&PathBuf::from("big.txt")));
}

#[test]
fn skips_files_with_null_byte() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join("bin.dat"), [b'h', 0x00, b'i']).unwrap();
    fs::write(root.join("text.txt"), "hello").unwrap();

    let files = scan(&ScanConfig {
        root: root.to_path_buf(),
    });
    let paths = rel_names(&files);
    assert!(paths.contains(&PathBuf::from("text.txt")));
    assert!(!paths.contains(&PathBuf::from("bin.dat")));
}

#[test]
fn skips_invalid_utf8() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // 0xFF は UTF-8 として不正
    fs::write(root.join("bad.txt"), [0x48, 0xFF, 0x49]).unwrap();
    fs::write(root.join("good.txt"), "hi").unwrap();

    let files = scan(&ScanConfig {
        root: root.to_path_buf(),
    });
    let paths = rel_names(&files);
    assert!(paths.contains(&PathBuf::from("good.txt")));
    assert!(!paths.contains(&PathBuf::from("bad.txt")));
}

#[test]
fn returns_deterministic_order() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    for name in ["c.txt", "a.txt", "b.txt"] {
        fs::write(root.join(name), "x").unwrap();
    }

    let files1 = scan(&ScanConfig {
        root: root.to_path_buf(),
    });
    let files2 = scan(&ScanConfig {
        root: root.to_path_buf(),
    });
    assert_eq!(files1, files2);
    assert_eq!(
        rel_names(&files1),
        vec![
            PathBuf::from("a.txt"),
            PathBuf::from("b.txt"),
            PathBuf::from("c.txt"),
        ]
    );
}

#[test]
fn scans_nested_directories() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let nested = root.join("a/b/c");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("deep.txt"), "x").unwrap();
    fs::write(root.join("shallow.txt"), "y").unwrap();

    let files = scan(&ScanConfig {
        root: root.to_path_buf(),
    });
    let paths = rel_names(&files);
    assert!(paths.contains(&PathBuf::from("shallow.txt")));
    assert!(paths.contains(&PathBuf::from("a/b/c/deep.txt")));
}
