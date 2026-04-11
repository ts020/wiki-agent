use std::path::PathBuf;

pub struct WikiNode {
    pub title: String,
    pub output_path: PathBuf,
    pub summary: String,
    pub key_files: Vec<KeyFile>,
    pub responsibilities: Vec<String>,
    pub related: Vec<PathBuf>,
    pub read_next: Vec<PathBuf>,
}

pub struct KeyFile {
    pub path: PathBuf,
    pub description: String,
}

pub struct ScanResult {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
}
