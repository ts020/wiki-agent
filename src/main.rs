use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use md_wiki::build::build_nodes;
use md_wiki::link::resolve_all;
use md_wiki::notes::ingest_notes;
use md_wiki::relations::compute_relations;
use md_wiki::render::tags::build_tag_index;
use md_wiki::render::{WikiOutput, write_wiki};
use md_wiki::scan::{ScanConfig, scan, scan_single_file};

const NOTE_COUNT_WARN: usize = 5_000;

#[derive(Parser, Debug)]
#[command(
    name = "md-wiki",
    version,
    about = "Markdown ファイルを投げ込むと育つ個人 wiki ジェネレータ"
)]
struct Cli {
    /// 入力（`.md` ファイル、またはディレクトリ）
    input: PathBuf,

    /// ディレクトリ入力時に再帰的に走査する
    #[arg(short, long)]
    recursive: bool,

    /// 出力先ディレクトリ
    #[arg(short, long, default_value = "./md-wiki")]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    if !cli.input.exists() {
        anyhow::bail!("input does not exist: {}", cli.input.display());
    }

    let out_abs = std::path::absolute(&cli.out).unwrap_or_else(|_| cli.out.clone());

    let (root, files) = if cli.input.is_file() {
        if cli.input.extension().and_then(|s| s.to_str()) != Some("md") {
            anyhow::bail!(
                "file input must have .md extension: {}",
                cli.input.display()
            );
        }
        let parent = cli
            .input
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let files = scan_single_file(&cli.input).into_iter().collect();
        (parent, files)
    } else {
        let files = scan(&ScanConfig {
            root: cli.input.clone(),
            extra_excluded: vec![out_abs.clone()],
            recursive: cli.recursive,
        });
        (cli.input.clone(), files)
    };

    let project_title = if cli.input.is_file() {
        cli.input
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from)
            .unwrap_or_else(|| "md-wiki".to_string())
    } else {
        root.canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "md-wiki".to_string())
    };

    let notes_data = ingest_notes(&files, &root);
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
    write_wiki(
        &cli.out,
        &WikiOutput {
            project_title: &project_title,
            nodes: &nodes,
            unresolved: &unresolved,
            graph: &graph,
        },
    )?;

    tracing::info!(
        input = %cli.input.display(),
        output = %cli.out.display(),
        files = files.len(),
        notes = nodes.len(),
        unresolved_links = unresolved.len(),
        "md-wiki generation complete"
    );
    Ok(())
}
