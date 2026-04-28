use std::path::PathBuf;

use anyhow::{Context, Result};
use md_wiki::large_md_gate::{GateMode, GateOptions, run_gate};

fn main() -> Result<()> {
    let args = parse_args()?;
    let report = run_gate(&args)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn parse_args() -> Result<GateOptions> {
    let mut mode = GateMode::Normal;
    let mut work_dir = PathBuf::from("target/large-md-gate");
    let mut report_path = None;
    let mut min_score = None;
    let mut require_resource_budget = false;
    let mut fixture_bytes_override = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--mode" => {
                mode = match next_value(&mut it, "--mode")?.as_str() {
                    "normal" => GateMode::Normal,
                    "heavy" => GateMode::Heavy,
                    other => anyhow::bail!("--mode must be normal or heavy, got {other}"),
                };
            }
            "--work-dir" => {
                work_dir = PathBuf::from(next_value(&mut it, "--work-dir")?);
            }
            "--report" => {
                report_path = Some(PathBuf::from(next_value(&mut it, "--report")?));
            }
            "--min-score" => {
                let raw = next_value(&mut it, "--min-score")?;
                min_score = Some(raw.parse::<f64>().context("--min-score must be a number")?);
            }
            "--require-resource-budget" => {
                require_resource_budget = true;
            }
            "--fixture-bytes" => {
                let raw = next_value(&mut it, "--fixture-bytes")?;
                fixture_bytes_override = Some(
                    raw.parse::<usize>()
                        .context("--fixture-bytes must be an integer")?,
                );
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }

    Ok(GateOptions {
        mode,
        work_dir,
        report_path,
        min_score,
        require_resource_budget,
        fixture_bytes_override,
    })
}

fn next_value(it: &mut impl Iterator<Item = String>, name: &str) -> Result<String> {
    it.next()
        .with_context(|| format!("{name} requires a value"))
}

fn print_help() {
    println!(
        "large_md_gate\n\n\
         Usage:\n\
           cargo run --example large_md_gate -- --mode normal --work-dir DIR --report FILE [--min-score N]\n\n\
         Options:\n\
           --mode normal|heavy          Gate mode. Default: normal\n\
           --work-dir DIR               Temporary fixture/output dir. Default: target/large-md-gate\n\
           --report FILE                Write machine-readable JSON report\n\
           --min-score N                Exit non-zero if score is below N\n\
           --require-resource-budget    Require heavy resource budget checks\n\
           --fixture-bytes N            Override fixture size for local smoke tests\n"
    );
}
