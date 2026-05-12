use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

use md_wiki::agentic_output::{finalize_agentic_plan, plan_large_markdown_pages};
use md_wiki::build::build_nodes;
use md_wiki::input_classifier::{InputKind, classify_scanned};
use md_wiki::link::resolve_all;
use md_wiki::notes::ingest_notes;
use md_wiki::output_plan::{
    Manifest, ManifestInputKind, OutputLock, OutputPlan, apply_incremental, read_manifest,
    source_hashes, write_manifest, write_plan_to_clean_dir,
};
use md_wiki::relations::compute_relations;
use md_wiki::render::tags::build_tag_index;
use md_wiki::render::{WikiOutput, build_core_wiki_plan};
use md_wiki::scan::{ScanConfig, ScannedFile, scan, scan_single_file};

const NOTE_COUNT_WARN: usize = 5_000;

#[derive(Parser, Debug)]
#[command(
    name = "md-wiki",
    version,
    about = "Markdown ファイルを init/add で育てる個人 wiki ジェネレータ"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 入力 Markdown から wiki を初期生成する
    Init(InitArgs),
    /// 初期化済み wiki を入力 root の現在状態へ差分更新する
    Add(AddArgs),
}

#[derive(Args, Debug)]
struct InitArgs {
    /// 入力（`.md` ファイル、またはディレクトリ）
    input: PathBuf,

    /// ディレクトリ入力時に再帰的に走査する
    #[arg(short, long)]
    recursive: bool,

    /// 出力先ディレクトリ
    #[arg(short, long, default_value = "./md-wiki")]
    out: PathBuf,
}

#[derive(Args, Debug)]
struct AddArgs {
    /// 入力 root 配下の追加確認対象。省略時は root 全体を再走査する
    path: Option<PathBuf>,

    /// 出力先ディレクトリ
    #[arg(short, long, default_value = "./md-wiki")]
    out: PathBuf,
}

struct PreparedGeneration {
    input_kind: ManifestInputKind,
    input_root: PathBuf,
    input_path: PathBuf,
    project_title: String,
    recursive: bool,
    files: Vec<ScannedFile>,
}

struct DesiredOutput {
    plan: OutputPlan,
    source_hashes: std::collections::BTreeMap<String, String>,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Init(args) => run_init(args),
        Command::Add(args) => run_add(args),
    }
}

fn run_init(args: InitArgs) -> anyhow::Result<()> {
    let _lock = OutputLock::acquire(&args.out)?;
    let prepared = prepare_generation(&args.input, args.recursive, &args.out)?;
    let desired = build_desired_output(&prepared)?;
    let manifest = Manifest::new(
        prepared.input_kind,
        &prepared.input_root,
        &prepared.input_path,
        prepared.recursive,
        desired.source_hashes,
        &desired.plan,
    );
    write_plan_to_clean_dir(&args.out, &desired.plan)?;
    write_manifest(&args.out, &manifest)?;

    tracing::info!(
        input = %prepared.input_path.display(),
        output = %args.out.display(),
        files = prepared.files.len(),
        generated_files = manifest.generated_file_hashes.len(),
        "md-wiki init complete"
    );
    Ok(())
}

fn run_add(args: AddArgs) -> anyhow::Result<()> {
    let _lock = OutputLock::acquire(&args.out)?;
    let manifest = read_manifest(&args.out)?;
    validate_add_path(args.path.as_deref(), &manifest)?;

    let input_path = PathBuf::from(&manifest.input_path);
    let prepared = prepare_generation_for_add(&manifest, &input_path, &args.out)?;
    if prepared.input_kind != manifest.input_kind {
        anyhow::bail!("manifest input kind no longer matches current input path");
    }

    let desired = build_desired_output(&prepared)?;
    apply_incremental(&args.out, &desired.plan, &manifest)?;

    let updated_manifest = Manifest::new(
        prepared.input_kind,
        &prepared.input_root,
        &prepared.input_path,
        prepared.recursive,
        desired.source_hashes,
        &desired.plan,
    );
    write_manifest(&args.out, &updated_manifest)?;

    tracing::info!(
        input = %prepared.input_path.display(),
        output = %args.out.display(),
        files = prepared.files.len(),
        generated_files = updated_manifest.generated_file_hashes.len(),
        "md-wiki add complete"
    );
    Ok(())
}

fn prepare_generation_for_add(
    manifest: &Manifest,
    input_path: &Path,
    output: &Path,
) -> anyhow::Result<PreparedGeneration> {
    if input_path.exists() {
        return prepare_generation(input_path, manifest.recursive, output);
    }
    if manifest.input_kind != ManifestInputKind::File {
        anyhow::bail!("input does not exist: {}", input_path.display());
    }

    let input_root = PathBuf::from(&manifest.input_root);
    let project_title = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(String::from)
        .unwrap_or_else(|| "md-wiki".to_string());
    Ok(PreparedGeneration {
        input_kind: ManifestInputKind::File,
        input_root,
        input_path: input_path.to_path_buf(),
        project_title,
        recursive: manifest.recursive,
        files: Vec::new(),
    })
}

fn prepare_generation(
    input: &Path,
    recursive: bool,
    output: &Path,
) -> anyhow::Result<PreparedGeneration> {
    if !input.exists() {
        anyhow::bail!("input does not exist: {}", input.display());
    }

    let input_abs = input.canonicalize()?;
    let out_abs = normalized_output_exclusion(output);

    if input_abs.is_file() {
        if input_abs.extension().and_then(|s| s.to_str()) != Some("md") {
            anyhow::bail!("file input must have .md extension: {}", input.display());
        }
        let input_root = input_abs
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let files = scan_single_file(&input_abs).into_iter().collect();
        let project_title = input_abs
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from)
            .unwrap_or_else(|| "md-wiki".to_string());
        Ok(PreparedGeneration {
            input_kind: ManifestInputKind::File,
            input_root,
            input_path: input_abs,
            project_title,
            recursive,
            files,
        })
    } else {
        let files = scan(&ScanConfig {
            root: input_abs.clone(),
            extra_excluded: vec![out_abs],
            recursive,
        });
        let project_title = input_abs
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "md-wiki".to_string());
        Ok(PreparedGeneration {
            input_kind: ManifestInputKind::Directory,
            input_root: input_abs.clone(),
            input_path: input_abs,
            project_title,
            recursive,
            files,
        })
    }
}

fn normalized_output_exclusion(output: &Path) -> PathBuf {
    if let Ok(path) = output.canonicalize() {
        return path;
    }
    if let Some(parent) = output.parent()
        && let Ok(parent) = parent.canonicalize()
        && let Some(name) = output.file_name()
    {
        return parent.join(name);
    }
    std::path::absolute(output).unwrap_or_else(|_| output.to_path_buf())
}

fn build_desired_output(prepared: &PreparedGeneration) -> anyhow::Result<DesiredOutput> {
    let classified = classify_scanned(&prepared.input_root, &prepared.files);
    let regular_files: Vec<_> = prepared
        .files
        .iter()
        .filter(|file| {
            classified.iter().any(|class| {
                class.relative_path == file.relative_path
                    && class.kind == InputKind::RegularMarkdown
            })
        })
        .cloned()
        .collect();
    let large_files: Vec<_> = classified
        .iter()
        .filter(|class| class.kind == InputKind::LargeMarkdown)
        .map(|class| class.relative_path.clone())
        .collect();

    let notes_data = ingest_notes(&regular_files, &prepared.input_root);
    if notes_data.len() > NOTE_COUNT_WARN {
        tracing::warn!(
            notes = notes_data.len(),
            "ingested notes exceed {NOTE_COUNT_WARN}, continuing"
        );
    }

    let mut nodes = build_nodes(notes_data);
    let (unresolved, graph) = resolve_all(&nodes);
    let tag_index = build_tag_index(&nodes);
    compute_relations(&mut nodes, &graph, &tag_index);

    let mut plan = build_core_wiki_plan(&WikiOutput {
        project_title: &prepared.project_title,
        nodes: &nodes,
        unresolved: &unresolved,
        graph: &graph,
    })?;
    plan_large_markdown_pages(&mut plan, &prepared.input_root, &large_files)?;
    finalize_agentic_plan(&mut plan)?;

    Ok(DesiredOutput {
        plan,
        source_hashes: source_hashes(&prepared.input_root, &prepared.files)?,
    })
}

fn validate_add_path(path: Option<&Path>, manifest: &Manifest) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    if !path.exists() {
        anyhow::bail!("add path does not exist: {}", path.display());
    }
    let add_abs = path.canonicalize()?;
    let input_root = PathBuf::from(&manifest.input_root);
    if !add_abs.starts_with(&input_root) {
        anyhow::bail!(
            "add path must be inside initialized input root: {}",
            input_root.display()
        );
    }
    if manifest.input_kind == ManifestInputKind::File
        && add_abs.as_path() != Path::new(&manifest.input_path)
    {
        anyhow::bail!("single-file wiki can only add the initialized file");
    }
    Ok(())
}
