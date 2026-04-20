use std::fs;

use repo_wiki::build::build_code_nodes;
use repo_wiki::extract::{detect_entry_points, detect_tech_stack, detect_test_layout};
use repo_wiki::render::{WikiOutput, write_wiki};
use repo_wiki::scan::{ScanConfig, scan};
use tempfile::TempDir;

fn run_generation(target: &std::path::Path, output: &std::path::Path, title: &str) {
    let files = scan(&ScanConfig {
        root: target.to_path_buf(),
        extra_excluded: Vec::new(),
    });
    let tech_stack = detect_tech_stack(&files, target);
    let entry_points = detect_entry_points(&files);
    let test_layout = detect_test_layout(&files);
    let nodes = build_code_nodes(&files);
    write_wiki(
        output,
        &WikiOutput {
            project_title: title,
            nodes: &nodes,
            tech_stack: &tech_stack,
            entry_points: &entry_points,
            test_layout: &test_layout,
        },
    )
    .unwrap();
}

#[test]
fn generates_index_overview_and_directory_nodes() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("src/scan")).unwrap();
    fs::write(
        target.join("Cargo.toml"),
        "[package]\nname = \"demo\"\n[dependencies]\nserde = \"1\"\n",
    )
    .unwrap();
    fs::write(target.join("README.md"), "hello").unwrap();
    fs::write(target.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(target.join("src/scan/mod.rs"), "pub fn scan() {}").unwrap();
    fs::create_dir(target.join("tests")).unwrap();
    fs::write(target.join("tests/it.rs"), "").unwrap();

    let output = tmp.path().join("out");
    run_generation(&target, &output, "project");

    let index = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(index.contains("# project"));
    assert!(index.contains("## Tech stack"));
    assert!(index.contains("Rust"));
    assert!(index.contains("## Overview"));
    assert!(index.contains("overview/tech-stack.md"));
    assert!(index.contains("development/index.md"));
    assert!(index.contains("directories/_root.md"));

    let tech = fs::read_to_string(output.join("overview/tech-stack.md")).unwrap();
    assert!(tech.contains("# Tech stack"));
    assert!(tech.contains("Cargo.toml"));
    assert!(tech.contains("serde"));

    let eps = fs::read_to_string(output.join("overview/entry-points.md")).unwrap();
    assert!(eps.contains("src/main.rs"));

    let tests = fs::read_to_string(output.join("overview/tests.md")).unwrap();
    assert!(tests.contains("tests"));

    let dev = fs::read_to_string(output.join("development/index.md")).unwrap();
    assert!(dev.contains("cargo build"));
}

#[test]
fn output_directory_inside_target_is_excluded_from_scan() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("code.rs"), "fn main() {}").unwrap();

    let output = target.join("repo-wiki");
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("index.md"), "old").unwrap();

    let output_abs = std::path::absolute(&output).unwrap();
    let files = scan(&ScanConfig {
        root: target.clone(),
        extra_excluded: vec![output_abs],
    });

    let paths: Vec<_> = files.iter().map(|f| f.relative_path.clone()).collect();
    assert!(paths.contains(&std::path::PathBuf::from("code.rs")));
    assert!(!paths.iter().any(|p| p.starts_with("repo-wiki")));
}

#[test]
fn rerun_clears_previous_output() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.txt"), "x").unwrap();

    let output = tmp.path().join("out");
    fs::create_dir_all(&output).unwrap();
    fs::write(output.join("stale.md"), "old").unwrap();

    run_generation(&target, &output, "project");

    assert!(!output.join("stale.md").exists());
    assert!(output.join("index.md").exists());
}
