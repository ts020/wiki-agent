use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

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

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let target = cli.target.unwrap_or_else(|| PathBuf::from("."));

    let files = scan(&ScanConfig {
        root: target.clone(),
    });

    tracing::info!(
        target = %target.display(),
        output = %cli.output.display(),
        files = files.len(),
        "scan complete (renderer not yet implemented)"
    );
}
