use std::path::{Path, PathBuf};

use crate::scan::ScannedFile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryPoint {
    pub file: PathBuf,
    pub language: &'static str,
    pub description: &'static str,
}

/// ルート直下および代表的な位置のエントリポイントらしきファイルを検出する。
pub fn detect_entry_points(scanned: &[ScannedFile]) -> Vec<EntryPoint> {
    let mut out = Vec::new();
    for f in scanned {
        if let Some(ep) = classify(&f.relative_path) {
            out.push(ep);
        }
    }
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out.dedup();
    out
}

fn classify(rel: &Path) -> Option<EntryPoint> {
    let rel_str = rel.to_str()?;
    let file_name = rel.file_name()?.to_str()?;

    // Rust
    if rel_str == "src/main.rs" {
        return Some(ep(rel, "Rust", "binary crate entry (src/main.rs)"));
    }
    if rel_str == "src/lib.rs" {
        return Some(ep(rel, "Rust", "library crate entry (src/lib.rs)"));
    }
    if rel_str.starts_with("src/bin/") && rel_str.ends_with(".rs") {
        return Some(ep(rel, "Rust", "bin target"));
    }

    // TypeScript / JavaScript
    if matches!(
        rel_str,
        "src/index.ts" | "src/index.tsx" | "src/index.js" | "src/index.jsx"
    ) {
        return Some(ep(rel, "TypeScript/JavaScript", "module index"));
    }
    if matches!(
        file_name,
        "index.ts" | "index.tsx" | "index.js" | "index.jsx"
    ) && rel
        .parent()
        .map(|p| p.as_os_str().is_empty())
        .unwrap_or(false)
    {
        return Some(ep(rel, "TypeScript/JavaScript", "root index"));
    }

    // Python
    if matches!(
        file_name,
        "__main__.py" | "main.py" | "app.py" | "manage.py"
    ) {
        return Some(ep(rel, "Python", "module/app entry"));
    }

    // Go
    if rel_str == "main.go" {
        return Some(ep(rel, "Go", "main package"));
    }
    if rel_str.starts_with("cmd/") && file_name == "main.go" {
        return Some(ep(rel, "Go", "cmd binary"));
    }

    None
}

fn ep(rel: &Path, language: &'static str, description: &'static str) -> EntryPoint {
    EntryPoint {
        file: rel.to_path_buf(),
        language,
        description,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::ScannedFile;

    fn sf(p: &str) -> ScannedFile {
        ScannedFile {
            relative_path: PathBuf::from(p),
            size: 0,
        }
    }

    #[test]
    fn detects_rust_main_and_lib() {
        let scanned = vec![sf("src/main.rs"), sf("src/lib.rs"), sf("src/foo.rs")];
        let eps = detect_entry_points(&scanned);
        let paths: Vec<_> = eps.iter().map(|e| e.file.clone()).collect();
        assert_eq!(
            paths,
            vec![PathBuf::from("src/lib.rs"), PathBuf::from("src/main.rs")]
        );
    }

    #[test]
    fn detects_python_entries() {
        let scanned = vec![sf("main.py"), sf("app.py"), sf("lib/helper.py")];
        let eps = detect_entry_points(&scanned);
        let names: Vec<_> = eps
            .iter()
            .map(|e| e.file.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"main.py".to_string()));
        assert!(names.contains(&"app.py".to_string()));
        assert!(!names.contains(&"helper.py".to_string()));
    }

    #[test]
    fn detects_go_cmd_bins() {
        let scanned = vec![sf("cmd/server/main.go"), sf("main.go")];
        let eps = detect_entry_points(&scanned);
        assert_eq!(eps.len(), 2);
    }

    #[test]
    fn detects_root_index_ts() {
        let scanned = vec![sf("index.ts"), sf("lib/index.ts")];
        let eps = detect_entry_points(&scanned);
        let paths: Vec<_> = eps.iter().map(|e| e.file.clone()).collect();
        assert_eq!(paths, vec![PathBuf::from("index.ts")]);
    }
}
