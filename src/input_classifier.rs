use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::scan::ScannedFile;

pub const LARGE_MARKDOWN_THRESHOLD_BYTES: u64 = 1024 * 1024;
pub const MAX_MANAGED_MARKDOWN_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputKind {
    RegularMarkdown,
    LargeMarkdown,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedInput {
    pub relative_path: PathBuf,
    pub size: u64,
    pub kind: InputKind,
    pub reason: Option<String>,
}

pub fn classify_scanned(root: &Path, files: &[ScannedFile]) -> Vec<ClassifiedInput> {
    files.iter().map(|file| classify_file(root, file)).collect()
}

pub fn classify_file(root: &Path, file: &ScannedFile) -> ClassifiedInput {
    if file.relative_path.extension().and_then(|s| s.to_str()) != Some("md") {
        return skipped(file, "not markdown");
    }

    let abs = root.join(&file.relative_path);
    if let Err(reason) = validate_utf8_markdown(&abs) {
        return skipped(file, &reason);
    }

    if file.size > MAX_MANAGED_MARKDOWN_BYTES {
        return ClassifiedInput {
            relative_path: file.relative_path.clone(),
            size: file.size,
            kind: InputKind::LargeMarkdown,
            reason: Some(
                "exceeds managed threshold; large path may require explicit budget".into(),
            ),
        };
    }

    let kind = if file.size > LARGE_MARKDOWN_THRESHOLD_BYTES {
        InputKind::LargeMarkdown
    } else {
        InputKind::RegularMarkdown
    };
    ClassifiedInput {
        relative_path: file.relative_path.clone(),
        size: file.size,
        kind,
        reason: None,
    }
}

fn skipped(file: &ScannedFile, reason: &str) -> ClassifiedInput {
    ClassifiedInput {
        relative_path: file.relative_path.clone(),
        size: file.size,
        kind: InputKind::Skipped,
        reason: Some(reason.to_string()),
    }
}

fn validate_utf8_markdown(path: &Path) -> Result<(), String> {
    let mut file = fs::File::open(path).map_err(|err| format!("failed to open: {err}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read: {err}"))?;
    if bytes.contains(&0) {
        return Err("contains NULL byte".into());
    }
    std::str::from_utf8(&bytes)
        .map(|_| ())
        .map_err(|err| format!("invalid UTF-8: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn scanned(path: &str, size: u64) -> ScannedFile {
        ScannedFile {
            relative_path: PathBuf::from(path),
            size,
        }
    }

    #[test]
    fn classifies_markdown_by_size_and_guards() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::write(root.join("regular.md"), "hello").unwrap();
        fs::write(root.join("large.md"), "a".repeat(1024 * 1024 + 1)).unwrap();
        fs::write(root.join("large.txt"), "a".repeat(1024 * 1024 + 1)).unwrap();
        fs::write(root.join("null.md"), [b'a', 0, b'b']).unwrap();
        fs::write(root.join("bad.md"), [0xff]).unwrap();

        assert_eq!(
            classify_file(root, &scanned("regular.md", 5)).kind,
            InputKind::RegularMarkdown
        );
        assert_eq!(
            classify_file(root, &scanned("large.md", 1024 * 1024 + 1)).kind,
            InputKind::LargeMarkdown
        );
        assert_eq!(
            classify_file(root, &scanned("large.txt", 1024 * 1024 + 1)).kind,
            InputKind::Skipped
        );
        assert_eq!(
            classify_file(root, &scanned("null.md", 3)).kind,
            InputKind::Skipped
        );
        assert_eq!(
            classify_file(root, &scanned("bad.md", 1)).kind,
            InputKind::Skipped
        );
    }
}
