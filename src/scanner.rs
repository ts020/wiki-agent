use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

use crate::types::ScanResult;

const EXCLUDED_DIRS: &[&str] = &[".git", "node_modules", "dist", "build", "target"];

const EXCLUDED_FILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
    "composer.lock",
    "Gemfile.lock",
    "poetry.lock",
    "go.sum",
];

pub fn scan(root: &Path) -> Result<ScanResult> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root).into_iter().filter_entry(|e| {
        // ルート自身は常に通す
        if e.depth() == 0 {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        if name.starts_with('.') {
            return false;
        }
        if e.file_type().is_dir() && EXCLUDED_DIRS.contains(&name.as_ref()) {
            return false;
        }
        true
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.file_type().is_file()
            && let Ok(rel) = entry.path().strip_prefix(root)
        {
            let name = entry.file_name().to_string_lossy();
            if EXCLUDED_FILES.contains(&name.as_ref()) {
                continue;
            }
            files.push(rel.to_path_buf());
        }
    }

    Ok(ScanResult {
        root: root.to_path_buf(),
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn excludes_git_directory() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::create_dir_all(tmp.path().join(".git/objects"))?;
        fs::write(tmp.path().join(".git/objects/dummy"), "x")?;
        fs::write(tmp.path().join("src.rs"), "fn main() {}")?;

        let result = scan(tmp.path())?;
        assert!(
            result
                .files
                .iter()
                .all(|p| !p.to_string_lossy().contains(".git"))
        );
        assert!(result.files.iter().any(|p| p == Path::new("src.rs")));
        Ok(())
    }

    #[test]
    fn excludes_lock_files() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join("Cargo.lock"), "lock")?;
        fs::write(tmp.path().join("package-lock.json"), "lock")?;
        fs::write(tmp.path().join("Cargo.toml"), "toml")?;

        let result = scan(tmp.path())?;
        assert!(
            result
                .files
                .iter()
                .all(|p| !p.to_string_lossy().contains("lock"))
        );
        assert!(result.files.iter().any(|p| p == Path::new("Cargo.toml")));
        Ok(())
    }

    #[test]
    fn paths_are_relative() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::create_dir_all(tmp.path().join("src"))?;
        fs::write(tmp.path().join("src/main.rs"), "fn main() {}")?;

        let result = scan(tmp.path())?;
        assert!(result.files.iter().any(|p| p == Path::new("src/main.rs")));
        Ok(())
    }
}
