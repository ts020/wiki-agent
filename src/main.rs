use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use repo_wiki::build::build_code_nodes;
use repo_wiki::render::write_wiki;
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

    let nodes = build_code_nodes(&files);
    write_wiki(&cli.output, &project_title, &nodes)?;

    tracing::info!(
        target = %target.display(),
        output = %cli.output.display(),
        files = files.len(),
        nodes = nodes.len(),
        "wiki generation complete"
    );
    Ok(())
}
