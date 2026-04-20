use std::fs;

use std::collections::HashSet;

use repo_wiki::build::{build_code_nodes_with, build_note_nodes};
use repo_wiki::extract::{detect_entry_points, detect_tech_stack, detect_test_layout};
use repo_wiki::link::resolve_all;
use repo_wiki::notes::ingest_notes;
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
    let notes = ingest_notes(&files, target);
    let mut used = HashSet::new();
    let mut nodes = build_code_nodes_with(&files, target, &mut used);
    nodes.extend(build_note_nodes(notes, &mut used));
    let unresolved = resolve_all(&mut nodes);
    write_wiki(
        output,
        &WikiOutput {
            project_title: title,
            nodes: &nodes,
            tech_stack: &tech_stack,
            entry_points: &entry_points,
            test_layout: &test_layout,
            unresolved: &unresolved,
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
fn ingests_notes_with_frontmatter_and_directory_convention() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("docs")).unwrap();
    fs::create_dir_all(target.join("src")).unwrap();

    fs::write(target.join("README.md"), "# Project\n\nRoot readme.").unwrap();
    fs::write(
        target.join("docs/architecture.md"),
        "---\ntitle: アーキテクチャ\ntags: [design]\n---\n\n# Overview\n\n本文。\n\n## Goals\n\n目標。",
    )
    .unwrap();
    fs::write(
        target.join("src/notes.md"),
        "---\nwiki: true\ntitle: Inline note\n---\n\nInline.",
    )
    .unwrap();
    fs::write(
        target.join("src/skip.md"),
        "---\nwiki: false\n---\nshould skip",
    )
    .unwrap();
    fs::write(target.join("src/ambient.md"), "# just sits here").unwrap(); // 取り込まれない

    let output = tmp.path().join("out");
    run_generation(&target, &output, "project");

    let index = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(index.contains("## Notes"));
    assert!(index.contains("notes/README.md"));
    assert!(index.contains("アーキテクチャ"));
    assert!(index.contains("Inline note"));
    assert!(!index.contains("ambient"));
    assert!(!index.contains("should skip"));

    // README の body が Content セクションに出ている
    let readme = fs::read_to_string(output.join("notes/README.md")).unwrap();
    assert!(readme.contains("## Content"));
    assert!(readme.contains("Root readme."));

    // docs/architecture.md は見出しツリーが Structure に出る
    let arch = fs::read_to_string(output.join("notes/docs/architecture.md")).unwrap();
    assert!(arch.contains("# アーキテクチャ"));
    assert!(arch.contains("Overview"));
    assert!(arch.contains("Goals"));

    // wiki:true の外部 md も取り込まれる
    assert!(output.join("notes/src/notes.md").exists());

    // wiki:false のファイルは除外
    assert!(!output.join("notes/src/skip.md").exists());
    assert!(!output.join("notes/src/ambient.md").exists());
}

#[test]
fn resolves_wikilinks_in_note_body_and_lists_unresolved() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("docs")).unwrap();

    fs::write(
        target.join("docs/index.md"),
        "links: [[architecture]], [[architecture#Goals]], [[architecture|plan]], ![[architecture]], [[missing]]",
    )
    .unwrap();
    fs::write(
        target.join("docs/architecture.md"),
        "# Arch\n\n## Goals\n\nGoals.",
    )
    .unwrap();

    let output = tmp.path().join("out");
    run_generation(&target, &output, "project");

    let idx = fs::read_to_string(output.join("notes/docs/index.md")).unwrap();
    assert!(idx.contains("[architecture](architecture.md)"));
    assert!(idx.contains("[architecture#Goals](architecture.md#goals)"));
    assert!(idx.contains("[plan](architecture.md)"));
    // embed は plain link に縮退する
    assert!(idx.contains("[architecture](architecture.md)"));
    // 未解決は原文 + (未解決)
    assert!(idx.contains("[[missing]] (未解決)"));

    let unresolved = fs::read_to_string(output.join("_unresolved.md")).unwrap();
    assert!(unresolved.contains("# Unresolved wikilinks"));
    assert!(unresolved.contains("missing"));

    let index_md = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(index_md.contains("Unresolved links"));
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
