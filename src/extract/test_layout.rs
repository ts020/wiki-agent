use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use crate::scan::ScannedFile;

#[derive(Debug, Default)]
pub struct TestLayout {
    pub test_dirs: BTreeSet<PathBuf>,
    pub test_files: Vec<PathBuf>,
}

const TEST_DIR_NAMES: &[&str] = &["tests", "test", "__tests__", "spec"];

pub fn detect_test_layout(scanned: &[ScannedFile]) -> TestLayout {
    let mut layout = TestLayout::default();
    for f in scanned {
        let rel = &f.relative_path;
        let in_test_dir = test_dir_ancestor(rel);
        if is_test_file(rel) || in_test_dir.is_some() {
            layout.test_files.push(rel.clone());
        }
        if let Some(dir) = in_test_dir {
            layout.test_dirs.insert(dir);
        }
    }
    layout.test_files.sort();
    layout.test_files.dedup();
    layout
}

fn is_test_file(rel: &Path) -> bool {
    let Some(name) = rel.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    // Rust
    if name.ends_with("_test.rs") || name.ends_with("_tests.rs") {
        return true;
    }
    // Go
    if name.ends_with("_test.go") {
        return true;
    }
    // JS/TS
    if name.ends_with(".test.ts")
        || name.ends_with(".test.tsx")
        || name.ends_with(".test.js")
        || name.ends_with(".test.jsx")
        || name.ends_with(".spec.ts")
        || name.ends_with(".spec.js")
    {
        return true;
    }
    // Python
    if (name.starts_with("test_") || name.ends_with("_test.py")) && name.ends_with(".py") {
        return true;
    }
    false
}

fn test_dir_ancestor(rel: &Path) -> Option<PathBuf> {
    let mut acc = PathBuf::new();
    for comp in rel.components() {
        if let Component::Normal(c) = comp {
            let s = c.to_str()?;
            acc.push(s);
            if TEST_DIR_NAMES.contains(&s) {
                return Some(acc.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sf(p: &str) -> ScannedFile {
        ScannedFile {
            relative_path: PathBuf::from(p),
            size: 0,
        }
    }

    #[test]
    fn detects_test_directories() {
        let scanned = vec![
            sf("tests/e2e.rs"),
            sf("src/foo/__tests__/bar.ts"),
            sf("src/main.rs"),
        ];
        let layout = detect_test_layout(&scanned);
        assert!(layout.test_dirs.contains(&PathBuf::from("tests")));
        assert!(
            layout
                .test_dirs
                .contains(&PathBuf::from("src/foo/__tests__"))
        );
        // test dir 配下のファイルは命名規則不問で test file 扱い
        assert!(layout.test_files.contains(&PathBuf::from("tests/e2e.rs")));
        assert!(
            layout
                .test_files
                .contains(&PathBuf::from("src/foo/__tests__/bar.ts"))
        );
    }

    #[test]
    fn detects_test_files_by_naming() {
        let scanned = vec![
            sf("foo_test.rs"),
            sf("foo.test.ts"),
            sf("test_bar.py"),
            sf("main.rs"),
            sf("lib.ts"),
        ];
        let layout = detect_test_layout(&scanned);
        assert_eq!(
            layout.test_files,
            vec![
                PathBuf::from("foo.test.ts"),
                PathBuf::from("foo_test.rs"),
                PathBuf::from("test_bar.py"),
            ]
        );
    }
}
