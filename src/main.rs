use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod analyzer;
mod generator;
mod llm;
mod scanner;
mod types;

#[derive(Parser)]
#[command(
    name = "repo-wiki-agent",
    about = "コードベースからMarkdown wikiツリーを生成する"
)]
struct Cli {
    /// 解析対象ディレクトリ（未指定時はカレントディレクトリ）
    #[arg(default_value = ".")]
    target_dir: PathBuf,

    /// 出力先ディレクトリ
    #[arg(short, long, default_value = "./repo-wiki")]
    output: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let target = cli.target_dir.canonicalize()?;

    let scan = scanner::scan(&target)?;
    let mut nodes = analyzer::analyze(&scan);
    generator::generate(&cli.output, &mut nodes, &target).await?;

    println!(
        "生成完了: {} ({} ノード)",
        cli.output.display(),
        nodes.len()
    );
    Ok(())
}
