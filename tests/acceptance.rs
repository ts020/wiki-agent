//! docs/要件定義/13-受け入れ基準.md の各項目に対応する E2E 受け入れテスト。

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use repo_wiki::build::{build_code_nodes_with, build_note_nodes};
use repo_wiki::extract::{detect_entry_points, detect_tech_stack, detect_test_layout};
use repo_wiki::link::resolve_all;
use repo_wiki::notes::ingest_notes;
use repo_wiki::relations::compute_relations;
use repo_wiki::render::tags::build_tag_index;
use repo_wiki::render::{WikiOutput, write_wiki};
use repo_wiki::scan::{ScanConfig, scan};
use tempfile::TempDir;

fn generate(target: &Path, output: &Path, title: &str) {
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
    let (unresolved, graph) = resolve_all(&mut nodes);
    let tag_index = build_tag_index(&nodes);
    compute_relations(&mut nodes, &graph, &tag_index);
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

fn snapshot_dir(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(path).unwrap().to_path_buf();
            let body = fs::read(entry.path()).unwrap();
            out.insert(rel, body);
        }
    }
    out
}

/// 受け入れ基準: index.md が生成される + 複数ノード + 除外 + コードベース非変更 + オフライン
#[test]
fn generates_index_multiple_nodes_and_excludes() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("src")).unwrap();
    fs::create_dir_all(target.join("node_modules")).unwrap();
    fs::create_dir_all(target.join("target/debug")).unwrap();
    fs::write(target.join("README.md"), "# project").unwrap();
    fs::write(target.join("src/a.rs"), "pub fn a() {}").unwrap();
    fs::write(target.join("src/b.rs"), "pub fn b() {}").unwrap();
    fs::write(target.join("node_modules/pkg.js"), "ignore me").unwrap();
    fs::write(target.join("target/debug/bin"), "ignore me").unwrap();

    let before = snapshot_dir(&target);
    let output = tmp.path().join("out");
    generate(&target, &output, "project");
    let after = snapshot_dir(&target);

    // コードベースが変更されない
    assert_eq!(before, after, "target ディレクトリが変更されている");
    // index.md
    assert!(output.join("index.md").exists());
    // 複数ノード
    let dir_count = fs::read_dir(output.join("directories"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .map(|x| x.path().extension() == Some("md".as_ref()))
                .unwrap_or(false)
        })
        .count();
    assert!(
        dir_count >= 2,
        "directories/ 配下に複数のノードが生成されるべき"
    );
    // 除外対象が無視される
    let index = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(!index.contains("node_modules"));
    assert!(!index.contains("target/debug"));
}

/// 受け入れ基準: ノート追加 → 再実行 → index/タグ/バックリンク更新
#[test]
fn rerun_picks_up_added_notes_and_updates_index() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("docs")).unwrap();
    fs::write(
        target.join("docs/a.md"),
        "---\ntitle: A\ntags: [x]\n---\n[[b]]",
    )
    .unwrap();

    let output = tmp.path().join("out");
    generate(&target, &output, "project");

    // 最初は b が未解決
    let idx1 = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(idx1.contains("Unresolved"));

    // b を追加して再実行
    fs::write(target.join("docs/b.md"), "---\ntitle: B\ntags: [x]\n---\n").unwrap();
    generate(&target, &output, "project");

    // 今度は解決され、backlink と tag 索引に反映される
    let idx2 = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(!idx2.contains("Unresolved"));
    assert!(idx2.contains("tags/x.md"));

    let b_md = fs::read_to_string(output.join("notes/docs/b.md")).unwrap();
    assert!(b_md.contains("## Backlinks"));
    assert!(b_md.contains("A"));
}

/// 受け入れ基準: `[[NoteName]]` 解決と `_unresolved.md` 列挙
#[test]
fn wikilinks_resolve_or_collect_unresolved() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("docs")).unwrap();
    fs::write(
        target.join("docs/here.md"),
        "links [[there]] and [[missing]]",
    )
    .unwrap();
    fs::write(target.join("docs/there.md"), "# There").unwrap();

    let output = tmp.path().join("out");
    generate(&target, &output, "project");

    let here = fs::read_to_string(output.join("notes/docs/here.md")).unwrap();
    assert!(here.contains("[there](there.md)"));
    assert!(here.contains("[[missing]] (未解決)"));

    let unresolved = fs::read_to_string(output.join("_unresolved.md")).unwrap();
    assert!(unresolved.contains("missing"));
    assert!(!unresolved.contains("| `notes/docs/there.md` |"));
}

/// 受け入れ基準: 100 件超ディレクトリで `_symbols.md` 生成
#[test]
fn generates_symbols_overflow_over_limit() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("src")).unwrap();
    // 110 個の pub fn を持つ 1 ファイル
    let mut body = String::new();
    for i in 0..110 {
        body.push_str(&format!("pub fn f{i}() {{}}\n"));
    }
    fs::write(target.join("src/big.rs"), &body).unwrap();

    let output = tmp.path().join("out");
    generate(&target, &output, "project");

    // `directories/src.md` は overflow 案内、`directories/src/_symbols.md` に全件
    let src_node = fs::read_to_string(output.join("directories/src.md")).unwrap();
    assert!(src_node.contains("_symbols.md"));
    let overflow = fs::read_to_string(output.join("directories/src/_symbols.md")).unwrap();
    assert!(overflow.contains("f0"));
    assert!(overflow.contains("f109"));
}

/// 受け入れ基準: 任意ノード単体で理解可能（構造項目が揃う）
#[test]
fn each_node_is_self_contained() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("src")).unwrap();
    fs::write(target.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    fs::write(target.join("src/lib.rs"), "pub fn foo() {}\npub struct S;").unwrap();
    fs::create_dir_all(target.join("docs")).unwrap();
    fs::write(
        target.join("docs/note.md"),
        "---\ntitle: Note\nsummary: Overview\n---\n# H\n\nbody.",
    )
    .unwrap();

    let output = tmp.path().join("out");
    generate(&target, &output, "project");

    let code = fs::read_to_string(output.join("directories/src.md")).unwrap();
    assert!(code.contains("## Summary"));
    assert!(code.contains("## Key files"));
    assert!(code.contains("## Structure"));

    let note = fs::read_to_string(output.join("notes/docs/note.md")).unwrap();
    assert!(note.contains("## Summary"));
    assert!(note.contains("## Key files"));
    assert!(note.contains("## Structure"));
    assert!(note.contains("## Content"));
}
