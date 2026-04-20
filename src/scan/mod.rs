use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

const MAX_FILE_SIZE: u64 = 1024 * 1024;
const PEEK_BYTES: usize = 8192;
const FILE_COUNT_WARN: usize = 10_000;
const DEPTH_WARN: usize = 20;

const EXCLUDED_DIRS: &[&str] = &[".git", "node_modules", "dist", "build", "target"];
const WIKI_HIDDEN_DIR: &str = ".wiki";

pub struct ScanConfig {
    pub root: PathBuf,
    /// 追加で除外したい絶対パス（出力先ディレクトリなど）
    pub extra_excluded: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    pub relative_path: PathBuf,
    pub size: u64,
}

pub fn scan(config: &ScanConfig) -> Vec<ScannedFile> {
    let root = &config.root;
    let mut files: Vec<ScannedFile> = Vec::new();
    let mut warned_count = false;
    let mut warned_depth = false;

    let extra_excluded: Vec<PathBuf> = config
        .extra_excluded
        .iter()
        .filter_map(|p| std::path::absolute(p).ok())
        .collect();

    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !should_prune(e, &extra_excluded));

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!(error = %err, "walk error, skipping subtree");
                continue;
            }
        };

        if !warned_depth && entry.depth() > DEPTH_WARN {
            tracing::warn!(
                depth = entry.depth(),
                path = %entry.path().display(),
                "directory depth exceeds {DEPTH_WARN}"
            );
            warned_depth = true;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_path_buf();

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(err) => {
                tracing::warn!(path = %rel.display(), error = %err, "failed to read metadata");
                continue;
            }
        };

        if metadata.len() > MAX_FILE_SIZE {
            tracing::warn!(
                path = %rel.display(),
                size = metadata.len(),
                "file exceeds 1 MiB, skipping"
            );
            continue;
        }

        if !is_probably_text(entry.path(), &rel) {
            continue;
        }

        files.push(ScannedFile {
            relative_path: rel,
            size: metadata.len(),
        });

        if !warned_count && files.len() >= FILE_COUNT_WARN {
            tracing::warn!(
                count = files.len(),
                "scanned files reached {FILE_COUNT_WARN}, continuing"
            );
            warned_count = true;
        }
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files
}

fn should_prune(entry: &DirEntry, extra_excluded: &[PathBuf]) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    if entry.file_type().is_dir()
        && let Ok(abs) = std::path::absolute(entry.path())
        && extra_excluded.contains(&abs)
    {
        return true;
    }
    let Some(name) = entry.file_name().to_str() else {
        return false;
    };
    // 除外名はファイル・ディレクトリ問わず遮断する
    // (git worktree の `.git` ファイル等を拾わないため)
    if EXCLUDED_DIRS.contains(&name) {
        return true;
    }
    if entry.file_type().is_dir() && name.starts_with('.') && name != WIKI_HIDDEN_DIR {
        return true;
    }
    false
}

fn is_probably_text(path: &Path, rel: &Path) -> bool {
    let mut buf = [0u8; PEEK_BYTES];
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(err) => {
            tracing::warn!(path = %rel.display(), error = %err, "failed to open file");
            return false;
        }
    };
    let n = match file.read(&mut buf) {
        Ok(n) => n,
        Err(err) => {
            tracing::warn!(path = %rel.display(), error = %err, "failed to read file head");
            return false;
        }
    };
    let head = &buf[..n];
    if head.contains(&0) {
        tracing::warn!(path = %rel.display(), "contains NULL byte, treating as binary");
        return false;
    }
    if let Err(err) = std::str::from_utf8(head) {
        // 境界でマルチバイトが切れた場合のみ許容する
        if err.error_len().is_some() {
            tracing::warn!(
                path = %rel.display(),
                error = %err,
                "not valid UTF-8, skipping"
            );
            return false;
        }
    }
    true
}
