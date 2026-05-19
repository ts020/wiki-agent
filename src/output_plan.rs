use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::metadata_renderer::markdown_path;

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const MANIFEST_PATH: &str = ".md-wiki/manifest.json";

pub type OutputPlan = BTreeMap<PathBuf, Vec<u8>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestInputKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub tool_version: String,
    pub input_kind: ManifestInputKind,
    pub input_root: String,
    pub input_path: String,
    pub recursive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<ManifestSchema>,
    pub source_hashes: BTreeMap<String, String>,
    pub generated_file_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSchema {
    pub path: String,
    pub hash: String,
}

pub struct OutputLock {
    path: PathBuf,
    _file: fs::File,
}

impl OutputLock {
    pub fn acquire(output_root: &Path) -> Result<Self> {
        let path = output_lock_path(output_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|err| {
                if err.kind() == std::io::ErrorKind::AlreadyExists {
                    let holder = fs::read_to_string(&path).unwrap_or_default();
                    anyhow::anyhow!(
                        "output is locked by another md-wiki process: {}{}",
                        path.display(),
                        if holder.trim().is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", holder.trim())
                        }
                    )
                } else {
                    anyhow::anyhow!("failed to acquire output lock {}: {err}", path.display())
                }
            })?;
        writeln!(file, "pid={}", std::process::id())
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(Self { path, _file: file })
    }
}

impl Drop for OutputLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl Manifest {
    pub fn new(
        input_kind: ManifestInputKind,
        input_root: &Path,
        input_path: &Path,
        recursive: bool,
        schema: Option<ManifestSchema>,
        source_hashes: BTreeMap<String, String>,
        plan: &OutputPlan,
    ) -> Self {
        Self {
            schema_version: MANIFEST_SCHEMA_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").into(),
            input_kind,
            input_root: markdown_path(input_root),
            input_path: markdown_path(input_path),
            recursive,
            schema,
            source_hashes,
            generated_file_hashes: plan_hashes(plan),
        }
    }
}

pub fn schema_manifest(path: &Path) -> Result<ManifestSchema> {
    let body = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    Ok(ManifestSchema {
        path: markdown_path(&path),
        hash: stable_hash(&body),
    })
}

pub fn validate_manifest_schema(schema: &ManifestSchema) -> Result<()> {
    let path = Path::new(&schema.path);
    let current = schema_manifest(path)?;
    if current.hash != schema.hash {
        anyhow::bail!(
            "schema pack changed since init/add: {}; rerun add with --schema to update schema artifacts",
            schema.path
        );
    }
    Ok(())
}

fn output_lock_path(output_root: &Path) -> PathBuf {
    let output_abs = normalized_lock_target(output_root);
    let hash = stable_hash(markdown_path(&output_abs).as_bytes());
    std::env::temp_dir()
        .join("md-wiki-locks")
        .join(format!("{hash}.lock"))
}

fn normalized_lock_target(output_root: &Path) -> PathBuf {
    if let Ok(path) = output_root.canonicalize() {
        return path;
    }
    if let Some(parent) = output_root.parent()
        && let Ok(parent) = parent.canonicalize()
        && let Some(name) = output_root.file_name()
    {
        return parent.join(name);
    }
    std::path::absolute(output_root).unwrap_or_else(|_| output_root.to_path_buf())
}

pub fn read_manifest(output_root: &Path) -> Result<Manifest> {
    let path = output_root.join(MANIFEST_PATH);
    let body =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: Manifest = serde_json::from_str(&body)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported md-wiki manifest schema version: {}",
            manifest.schema_version
        );
    }
    validate_manifest_generated_paths(&manifest)?;
    Ok(manifest)
}

pub fn write_manifest(output_root: &Path, manifest: &Manifest) -> Result<()> {
    let path = output_root.join(MANIFEST_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let body = serde_json::to_vec_pretty(manifest)?;
    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))
}

pub fn insert_text(plan: &mut OutputPlan, rel: impl Into<PathBuf>, body: impl AsRef<str>) {
    plan.insert(rel.into(), body.as_ref().as_bytes().to_vec());
}

pub fn insert_bytes(plan: &mut OutputPlan, rel: impl Into<PathBuf>, body: Vec<u8>) {
    plan.insert(rel.into(), body);
}

pub fn collect_markdown_plan_from_dir(root: &Path) -> Result<OutputPlan> {
    let mut plan = OutputPlan::new();
    if !root.exists() {
        return Ok(plan);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                let rel = path.strip_prefix(root)?.to_path_buf();
                plan.insert(rel, fs::read(&path)?);
            }
        }
    }
    Ok(plan)
}

pub fn write_plan_to_clean_dir(output_root: &Path, plan: &OutputPlan) -> Result<()> {
    clean_output(output_root)?;
    write_plan_files(output_root, plan)
}

pub fn write_plan_files(output_root: &Path, plan: &OutputPlan) -> Result<()> {
    for (rel, body) in plan {
        write_file(output_root, rel, body)?;
    }
    Ok(())
}

pub fn apply_incremental(
    output_root: &Path,
    desired: &OutputPlan,
    previous: &Manifest,
) -> Result<()> {
    if !output_root.exists() {
        fs::create_dir_all(output_root)
            .with_context(|| format!("failed to create {}", output_root.display()))?;
    }
    if !output_root.is_dir() {
        anyhow::bail!(
            "output path exists but is not a directory: {}",
            output_root.display()
        );
    }

    let managed_parent_blockers = preflight_incremental(output_root, desired, previous)?;

    for rel in managed_parent_blockers {
        let abs = output_root.join(&rel);
        if fs::symlink_metadata(&abs).is_ok() {
            fs::remove_file(&abs).with_context(|| format!("failed to remove {}", abs.display()))?;
        }
    }

    for (rel, body) in desired {
        let abs = output_root.join(rel);
        let current = read_existing_regular_file(&abs)?;
        if current.as_deref() == Some(body.as_slice()) {
            continue;
        }
        write_file(output_root, rel, body)?;
    }

    for rel_key in previous.generated_file_hashes.keys() {
        validate_relative_output_path(Path::new(rel_key))?;
        let rel = PathBuf::from(rel_key);
        if desired.contains_key(&rel) {
            continue;
        }
        let abs = output_root.join(&rel);
        if fs::symlink_metadata(&abs).is_ok() {
            fs::remove_file(&abs).with_context(|| format!("failed to remove {}", abs.display()))?;
            prune_empty_parents(output_root, rel.parent())?;
        }
    }

    Ok(())
}

fn preflight_incremental(
    output_root: &Path,
    desired: &OutputPlan,
    previous: &Manifest,
) -> Result<BTreeSet<PathBuf>> {
    let mut managed_parent_blockers = BTreeSet::new();
    for rel in desired.keys() {
        validate_relative_output_path(rel)?;
        let rel_key = markdown_path(rel);
        let abs = output_root.join(rel);
        if let Some(meta) = optional_symlink_metadata(&abs)? {
            if !previous.generated_file_hashes.contains_key(&rel_key) {
                anyhow::bail!(
                    "refusing to overwrite unmanaged file in output: {}",
                    abs.display()
                );
            }
            if meta.file_type().is_symlink() {
                anyhow::bail!("refusing to follow symlink output path: {}", abs.display());
            }
            if !meta.file_type().is_file() {
                anyhow::bail!(
                    "refusing to overwrite non-file output path: {}",
                    abs.display()
                );
            }
        }
        preflight_parent_dirs(
            output_root,
            rel,
            desired,
            previous,
            &mut managed_parent_blockers,
        )?;
    }

    for rel_key in previous.generated_file_hashes.keys() {
        validate_relative_output_path(Path::new(rel_key))?;
        let rel = PathBuf::from(rel_key);
        if desired.contains_key(&rel) {
            continue;
        }
        let abs = output_root.join(&rel);
        if let Some(meta) = optional_symlink_metadata(&abs)?
            && meta.file_type().is_dir()
        {
            anyhow::bail!(
                "refusing to remove managed output path that is not a file: {}",
                abs.display()
            );
        }
    }

    Ok(managed_parent_blockers)
}

fn preflight_parent_dirs(
    output_root: &Path,
    rel: &Path,
    desired: &OutputPlan,
    previous: &Manifest,
    managed_parent_blockers: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let Some(parent) = rel.parent() else {
        return Ok(());
    };
    let mut current = PathBuf::new();
    for component in parent.components() {
        current.push(component.as_os_str());
        let abs = output_root.join(&current);
        if let Some(meta) = optional_symlink_metadata(&abs)? {
            if meta.file_type().is_dir() && !meta.file_type().is_symlink() {
                continue;
            }
            let current_key = markdown_path(&current);
            if meta.file_type().is_file()
                && previous.generated_file_hashes.contains_key(&current_key)
                && !desired.contains_key(&current)
            {
                managed_parent_blockers.insert(current.clone());
                continue;
            }
            anyhow::bail!(
                "refusing to create generated file because parent path is not a directory: {}",
                abs.display()
            );
        }
    }
    Ok(())
}

fn validate_manifest_generated_paths(manifest: &Manifest) -> Result<()> {
    for rel_key in manifest.generated_file_hashes.keys() {
        validate_relative_output_path(Path::new(rel_key))?;
    }
    Ok(())
}

fn validate_relative_output_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        anyhow::bail!("invalid generated output path in manifest: empty path");
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            anyhow::bail!(
                "invalid generated output path in manifest: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn optional_symlink_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta)),
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn read_existing_regular_file(path: &Path) -> Result<Option<Vec<u8>>> {
    let Some(meta) = optional_symlink_metadata(path)? else {
        return Ok(None);
    };
    if meta.file_type().is_symlink() {
        anyhow::bail!("refusing to follow symlink output path: {}", path.display());
    }
    if !meta.file_type().is_file() {
        anyhow::bail!("output path exists but is not a file: {}", path.display());
    }
    fs::read(path)
        .map(Some)
        .with_context(|| format!("failed to read {}", path.display()))
}

pub fn plan_hashes(plan: &OutputPlan) -> BTreeMap<String, String> {
    plan.iter()
        .map(|(rel, body)| (markdown_path(rel), stable_hash(body)))
        .collect()
}

pub fn source_hashes(
    root: &Path,
    files: &[crate::scan::ScannedFile],
) -> Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for file in files {
        let abs = root.join(&file.relative_path);
        let body = fs::read(&abs).with_context(|| format!("failed to read {}", abs.display()))?;
        hashes.insert(markdown_path(&file.relative_path), stable_hash(&body));
    }
    Ok(hashes)
}

pub fn stable_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn clean_output(output_root: &Path) -> Result<()> {
    if !output_root.exists() {
        return Ok(());
    }
    if !output_root.is_dir() {
        anyhow::bail!(
            "output path exists but is not a directory: {}",
            output_root.display()
        );
    }

    let is_empty = fs::read_dir(output_root)
        .map(|mut it| it.next().is_none())
        .unwrap_or(false);
    if is_empty {
        return Ok(());
    }

    let looks_like_ours = output_root.join(MANIFEST_PATH).exists()
        || output_root.join("index.md").exists()
        || output_root.join("fragments").exists();
    if !looks_like_ours {
        anyhow::bail!(
            "refusing to clean {}: does not look like a md-wiki output directory \
             (no manifest, index.md, or fragments/). Remove it manually or choose a different --out.",
            output_root.display()
        );
    }

    fs::remove_dir_all(output_root)
        .with_context(|| format!("failed to clear {}", output_root.display()))?;
    Ok(())
}

fn write_file(root: &Path, rel: &Path, body: &[u8]) -> Result<()> {
    validate_relative_output_path(rel)?;
    ensure_output_root_dir(root)?;
    let abs = root.join(rel);
    if abs.parent().is_some() {
        ensure_parent_dirs(root, rel.parent())?;
    }
    if let Some(meta) = optional_symlink_metadata(&abs)?
        && meta.file_type().is_symlink()
    {
        anyhow::bail!("refusing to follow symlink output path: {}", abs.display());
    }
    fs::write(&abs, body).with_context(|| format!("failed to write {}", abs.display()))
}

fn ensure_output_root_dir(root: &Path) -> Result<()> {
    match optional_symlink_metadata(root)? {
        Some(meta) if meta.file_type().is_dir() && !meta.file_type().is_symlink() => Ok(()),
        Some(_) => anyhow::bail!(
            "output path exists but is not a directory: {}",
            root.display()
        ),
        None => {
            fs::create_dir_all(root)
                .with_context(|| format!("failed to create {}", root.display()))?;
            Ok(())
        }
    }
}

fn ensure_parent_dirs(root: &Path, first_parent: Option<&Path>) -> Result<()> {
    let Some(parent) = first_parent else {
        return Ok(());
    };
    let mut current = PathBuf::new();
    for component in parent.components() {
        current.push(component.as_os_str());
        let abs = root.join(&current);
        match optional_symlink_metadata(&abs)? {
            Some(meta) if meta.file_type().is_dir() && !meta.file_type().is_symlink() => {}
            Some(_) => {
                anyhow::bail!(
                    "refusing to create generated file because parent path is not a directory: {}",
                    abs.display()
                );
            }
            None => {
                fs::create_dir(&abs)
                    .with_context(|| format!("failed to create {}", abs.display()))?;
            }
        }
    }
    Ok(())
}

fn prune_empty_parents(root: &Path, first_parent: Option<&Path>) -> Result<()> {
    let mut current = first_parent.map(PathBuf::from);
    while let Some(rel) = current {
        if rel.as_os_str().is_empty() || rel == Path::new(".md-wiki") {
            break;
        }
        let abs = root.join(&rel);
        match fs::remove_dir(&abs) {
            Ok(()) => current = rel.parent().map(PathBuf::from),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                current = rel.parent().map(PathBuf::from);
            }
            Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(err) => {
                return Err(err).with_context(|| format!("failed to remove {}", abs.display()));
            }
        }
    }
    Ok(())
}
