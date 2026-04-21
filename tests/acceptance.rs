//! docs/要件定義/13-受け入れ基準.md の各項目に対応する E2E 受け入れテスト。
//! 断片化 (FR-05) に対応した構造で検証する。

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
            graph: &graph,
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

/// AC-02 / AC-05: ディレクトリ入力の基本と除外
#[test]
fn generates_index_and_entries_and_excludes() {
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

    // AC-02 index.md と入口ページ
    assert!(output.join("index.md").exists());
    assert!(output.join("fragments/README/index.md").exists());
    assert!(output.join("fragments/notes-a/index.md").exists());

    // AC-05 除外対象
    assert!(!output.join("fragments/.git").exists());
    assert!(!output.join("fragments/node_modules").exists());
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

    assert!(output.join("fragments/top/index.md").exists());
    assert!(!output.join("fragments/deep/nested/index.md").exists());
}

/// AC-04: `--recursive` 時はサブディレクトリの md も取り込まれ相対パスがディレクトリとして維持される
#[test]
fn recursive_preserves_nested_paths() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("a/b")).unwrap();
    fs::write(target.join("a/b/deep.md"), "# Deep").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    assert!(output.join("fragments/a/b/deep/index.md").exists());
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

    let here = fs::read_to_string(output.join("fragments/here/index.md")).unwrap();
    // 入口ページ → 入口ページのリンク
    assert!(
        here.contains("[there](../there/index.md)"),
        "解決済みリンクが出力に含まれること: {here}"
    );
    assert!(here.contains("[[missing]] (未解決)"));

    let unresolved = fs::read_to_string(output.join("_unresolved.md")).unwrap();
    assert!(unresolved.contains("missing"));
}

/// AC-06: `[[Foo#見出し]]` は該当 h2 の断片ページへ
#[test]
fn wikilink_with_heading_resolves_to_fragment_page() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("here.md"), "see [[there#Design]] for details").unwrap();
    fs::write(target.join("there.md"), "# There\n\n## Design\n\ndetails\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let here = fs::read_to_string(output.join("fragments/here/index.md")).unwrap();
    assert!(
        here.contains("../there/design.md"),
        "#見出し付きリンクが断片ページを指すこと: {here}"
    );
}

/// AC-09: 見出し索引。全ノートの h1-h2 が、対応する入口／断片ページへ張られる
#[test]
fn heading_index_links_to_entry_and_fragments() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("a.md"),
        "# Top\n\nbody\n\n## Sub section\n\nmore",
    )
    .unwrap();
    fs::write(target.join("b.md"), "# B-Top\n\n### Deep\n\n## B-Sub").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let headings = fs::read_to_string(output.join("headings/index.md")).unwrap();
    assert!(headings.contains("Top"));
    assert!(headings.contains("Sub section"));
    assert!(headings.contains("B-Sub"));
    // h3 は含めない（FR-08）
    assert!(!headings.contains("(../fragments/b/deep.md)"));
    // 断片ページへのリンクが張られていること
    assert!(headings.contains("../fragments/a/sub-section.md"));
    assert!(headings.contains("../fragments/b/b-sub.md"));
}

/// AC-10: リンク索引。ページ間の参照関係が列挙される
#[test]
fn link_index_lists_forward_references_per_page() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.md"), "# A\n\nsee [[b]] and [[c]]").unwrap();
    fs::write(target.join("b.md"), "# B\n\nsee [[c]]").unwrap();
    fs::write(target.join("c.md"), "# C").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let links = fs::read_to_string(output.join("links/index.md")).unwrap();
    // a の入口から b の入口、c の入口への参照
    assert!(links.contains("../fragments/a/index.md"));
    assert!(links.contains("../fragments/b/index.md"));
    assert!(links.contains("../fragments/c/index.md"));
}

/// tags/index.md が全タグの入口として生成される
#[test]
fn tags_index_lists_all_tags() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("a.md"),
        "---\ntags: [alpha, beta/sub]\n---\n# A",
    )
    .unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let index = fs::read_to_string(output.join("tags/index.md")).unwrap();
    assert!(index.contains("alpha"));
    assert!(index.contains("beta"));
    assert!(index.contains("beta/sub"));
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

/// AC-11: バックリンクが付与される（参照が無いページは `## Backlinks` を出さない）
#[test]
fn backlinks_appear_when_referenced() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.md"), "# A\n\nrefs [[b]]").unwrap();
    fs::write(target.join("b.md"), "# B").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let b = fs::read_to_string(output.join("fragments/b/index.md")).unwrap();
    assert!(b.contains("## Backlinks"));
    assert!(b.contains("A"));

    let a = fs::read_to_string(output.join("fragments/a/index.md")).unwrap();
    assert!(
        !a.contains("## Backlinks"),
        "参照されていないページでは Backlinks セクションを出さない"
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
    let a = fs::read_to_string(output.join("fragments/a/index.md")).unwrap();
    assert!(a.contains("[b](../b/index.md)"));
}

/// AC-16: h2 複数のノートでナビ（Parent / Prev / Next）が正しく張られる
#[test]
fn h2_fragments_have_correct_navigation() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("n.md"),
        "# N\n\n## Alpha\n\na\n\n## Bravo\n\nb\n\n## Charlie\n\nc\n",
    )
    .unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let alpha = fs::read_to_string(output.join("fragments/n/alpha.md")).unwrap();
    // 先頭: Prev 無し・Next あり
    assert!(alpha.contains("Parent: "));
    assert!(!alpha.contains("Prev:"));
    assert!(alpha.contains("Next: [Bravo](bravo.md)"));

    let bravo = fs::read_to_string(output.join("fragments/n/bravo.md")).unwrap();
    // 中間: Prev/Next 両方
    assert!(bravo.contains("Prev: [Alpha](alpha.md)"));
    assert!(bravo.contains("Next: [Charlie](charlie.md)"));

    let charlie = fs::read_to_string(output.join("fragments/n/charlie.md")).unwrap();
    // 末尾: Prev あり・Next 無し
    assert!(charlie.contains("Prev: [Bravo](bravo.md)"));
    assert!(!charlie.contains("Next:"));
}

/// AC-18: fragment: false は入口ページに全文を収め、断片ページを生成しない
#[test]
fn fragment_false_opts_out_of_fragmentation() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("n.md"),
        "---\nfragment: false\n---\n# N\n\n## Alpha\n\na\n\n## Bravo\n\nb\n",
    )
    .unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let entry = fs::read_to_string(output.join("fragments/n/index.md")).unwrap();
    assert!(entry.contains("## Alpha"), "本文が入口に残っていること");
    assert!(entry.contains("## Bravo"));
    assert!(!output.join("fragments/n/alpha.md").exists());
    assert!(!output.join("fragments/n/bravo.md").exists());
    assert!(
        !entry.contains("## Fragments"),
        "非断片化では Fragments セクションを出さない"
    );
}

/// AC-19: h2 が 1 個でも入口 + 断片 1 枚に分かれる
#[test]
fn single_h2_note_splits_entry_and_one_fragment() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("n.md"), "# N\n\nintro\n\n## Only\n\nx\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    assert!(output.join("fragments/n/index.md").exists());
    let only = fs::read_to_string(output.join("fragments/n/only.md")).unwrap();
    assert!(only.contains("Parent:"));
    assert!(!only.contains("Prev:"));
    assert!(!only.contains("Next:"));
}

/// AC-20: h2 が無いノートは入口のみで、断片ページは生成されない
#[test]
fn no_h2_note_has_entry_only() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("n.md"), "# N\n\nbody only.\n\n### deep\n\nx\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let entry = fs::read_to_string(output.join("fragments/n/index.md")).unwrap();
    assert!(entry.contains("body only."));
    // 断片ページは生成されない
    let fragments_dir = output.join("fragments/n");
    let files: Vec<_> = std::fs::read_dir(&fragments_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().ok().is_some_and(|t| t.is_file()))
        .map(|e| e.file_name())
        .collect();
    assert_eq!(files.len(), 1, "入口 index.md のみ");
}
