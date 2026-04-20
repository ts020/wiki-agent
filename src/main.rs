use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use repo_wiki::build::{build_code_nodes_with, build_note_nodes};
use repo_wiki::extract::{detect_entry_points, detect_tech_stack, detect_test_layout};
use repo_wiki::link::resolve_all;
use repo_wiki::notes::ingest_notes;
use repo_wiki::render::{WikiOutput, write_wiki};
use repo_wiki::scan::{ScanConfig, scan};

#[derive(Parser, Debug)]
#[command(
    name = "repo-wiki",
    version,
    about = "Generate an explorable Markdown wiki tree from a codebase"
)]
struct Cli {
    /// 走査対象ディレクトリ（未指定時はカレントディレクトリ）
    target: Option<PathBuf>,

    /// 出力先ディレクトリ
    #[arg(short, long, default_value = "./repo-wiki")]
    output: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let target = cli.target.unwrap_or_else(|| PathBuf::from("."));

    let output_abs = std::path::absolute(&cli.output).unwrap_or_else(|_| cli.output.clone());
    let files = scan(&ScanConfig {
        root: target.clone(),
        extra_excluded: vec![output_abs],
    });

    let project_title = target
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "repo-wiki".to_string());

    let tech_stack = detect_tech_stack(&files, &target);
    let entry_points = detect_entry_points(&files);
    let test_layout = detect_test_layout(&files);
    let notes_data = ingest_notes(&files, &target);

    let mut used_paths = std::collections::HashSet::new();
    let mut nodes = build_code_nodes_with(&files, &target, &mut used_paths);
    nodes.extend(build_note_nodes(notes_data, &mut used_paths));
    let unresolved = resolve_all(&mut nodes);
    write_wiki(
        &cli.output,
        &WikiOutput {
            project_title: &project_title,
            nodes: &nodes,
            tech_stack: &tech_stack,
            entry_points: &entry_points,
            test_layout: &test_layout,
            unresolved: &unresolved,
        },
    )?;

    let note_count = nodes
        .iter()
        .filter(|n| matches!(n.kind, repo_wiki::model::NodeKind::NoteDerived))
        .count();
    tracing::info!(
        target = %target.display(),
        output = %cli.output.display(),
        files = files.len(),
        nodes = nodes.len(),
        notes = note_count,
        languages = tech_stack.languages.len(),
        entry_points = entry_points.len(),
        unresolved_links = unresolved.len(),
        "wiki generation complete"
    );
    Ok(())
}
