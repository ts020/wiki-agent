//! docs/要件定義/13-受け入れ基準.md の各項目に対応する E2E 受け入れテスト。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use md_wiki::build::build_nodes;
use md_wiki::link::resolve_all;
use md_wiki::notes::ingest_notes;
use md_wiki::relations::compute_relations;
use md_wiki::render::tags::build_tag_index;
use md_wiki::render::{WikiOutput, write_wiki};
use md_wiki::scan::{ScanConfig, scan};
use tempfile::TempDir;

fn generate_dir(target: &Path, output: &Path, title: &str, recursive: bool) {
    let files = scan(&ScanConfig {
        root: target.to_path_buf(),
        extra_excluded: Vec::new(),
        recursive,
    });
    let notes = ingest_notes(&files, target);
    let mut nodes = build_nodes(notes);
    let (unresolved, graph) = resolve_all(&nodes);
    let tag_index = build_tag_index(&nodes);
    compute_relations(&mut nodes, &graph, &tag_index);
    write_wiki(
        output,
        &WikiOutput {
            project_title: title,
            nodes: &nodes,
            unresolved: &unresolved,
        },
    )
    .unwrap();
}

fn snapshot_dir(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    if !path.exists() {
        return out;
    }
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

/// AC-02 / AC-05: ディレクトリ入力（非再帰でも直下の md が拾われる）と除外
#[test]
fn generates_index_and_notes_and_excludes() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join(".git")).unwrap();
    fs::create_dir_all(target.join("node_modules")).unwrap();
    fs::write(target.join("README.md"), "# project\n\nbody").unwrap();
    fs::write(target.join("notes-a.md"), "# A").unwrap();
    fs::write(target.join(".git/HEAD"), "ref: ...").unwrap();
    fs::write(target.join("node_modules/pkg.md"), "ignore").unwrap();

    let before = snapshot_dir(&target);
    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);
    let after = snapshot_dir(&target);

    // AC-14 非破壊
    assert_eq!(before, after, "input directory が変更されている");

    // AC-02 index.md と notes/ が生成される
    assert!(output.join("index.md").exists());
    assert!(output.join("notes/README.md").exists());
    assert!(output.join("notes/notes-a.md").exists());

    // AC-05 除外対象
    assert!(!output.join("notes/.git").exists());
    assert!(!output.join("notes/node_modules").exists());
}

/// AC-03: 非再帰ではサブディレクトリは走査されない
#[test]
fn non_recursive_skips_subdirectories() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("deep")).unwrap();
    fs::write(target.join("top.md"), "# Top").unwrap();
    fs::write(target.join("deep/nested.md"), "# Nested").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", false);

    assert!(output.join("notes/top.md").exists());
    assert!(!output.join("notes/deep/nested.md").exists());
}

/// AC-04: `--recursive` 時はサブディレクトリの md も取り込まれ相対パスが維持される
#[test]
fn recursive_preserves_nested_paths() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("a/b")).unwrap();
    fs::write(target.join("a/b/deep.md"), "# Deep").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    assert!(output.join("notes/a/b/deep.md").exists());
}

/// AC-06 / AC-07: wikilink 解決と未解決リンク集約
#[test]
fn wikilinks_resolve_or_collect_unresolved() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("here.md"), "links [[there]] and [[missing]]").unwrap();
    fs::write(target.join("there.md"), "# There").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let here = fs::read_to_string(output.join("notes/here.md")).unwrap();
    assert!(
        here.contains("[there](there.md)"),
        "解決済みリンクが出力に含まれること: {here}"
    );
    assert!(here.contains("[[missing]] (未解決)"));

    let unresolved = fs::read_to_string(output.join("_unresolved.md")).unwrap();
    assert!(unresolved.contains("missing"));
}

/// AC-08: タグ索引。ネストタグは親・子の双方に登場する
#[test]
fn tag_index_and_nested_tags() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("a.md"),
        "---\ntitle: A\ntags: [auth/session]\n---\n# A",
    )
    .unwrap();
    fs::write(target.join("b.md"), "---\ntitle: B\ntags: [auth]\n---\n# B").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let auth = fs::read_to_string(output.join("tags/auth.md")).unwrap();
    assert!(auth.contains("A"));
    assert!(auth.contains("B"));
    let session = fs::read_to_string(output.join("tags/auth/session.md")).unwrap();
    assert!(session.contains("A"));
    assert!(!session.contains("- [B]"));
}

/// AC-11: バックリンクが付与される（参照が無いノートは `## Backlinks` を出さない）
#[test]
fn backlinks_appear_when_referenced() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.md"), "# A\n\nrefs [[b]]").unwrap();
    fs::write(target.join("b.md"), "# B").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let b = fs::read_to_string(output.join("notes/b.md")).unwrap();
    assert!(b.contains("## Backlinks"));
    assert!(b.contains("A"));

    let a = fs::read_to_string(output.join("notes/a.md")).unwrap();
    assert!(
        !a.contains("## Backlinks"),
        "参照されていないノートでは Backlinks セクションを出さない"
    );
}

/// AC-13 冪等性: 2 回連続実行で出力が一致する
#[test]
fn idempotent_on_rerun() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("docs")).unwrap();
    fs::write(
        target.join("docs/a.md"),
        "---\ntitle: A\ntags: [x]\n---\n[[b]]",
    )
    .unwrap();
    fs::write(target.join("docs/b.md"), "---\ntitle: B\ntags: [x]\n---\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);
    let first = snapshot_dir(&output);
    generate_dir(&target, &output, "project", true);
    let second = snapshot_dir(&output);

    assert_eq!(
        first, second,
        "同一入力に対して出力は毎回一致すること（FR-03 + AC-13）"
    );
}

/// AC-13 増築: ノートを追加して再実行すると index と unresolved が更新される
#[test]
fn rerun_picks_up_added_notes() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.md"), "links [[b]]").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let idx1 = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(idx1.contains("Unresolved"));

    fs::write(target.join("b.md"), "# B").unwrap();
    generate_dir(&target, &output, "project", true);

    let idx2 = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(!idx2.contains("## Unresolved"));
    let a = fs::read_to_string(output.join("notes/a.md")).unwrap();
    assert!(a.contains("[b](b.md)"));
}
