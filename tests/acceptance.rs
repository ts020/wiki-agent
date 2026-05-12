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

/// AC-04: 非再帰ではサブディレクトリは走査されない
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

/// AC-03: 既定の再帰走査ではサブディレクトリの md も取り込まれ相対パスがディレクトリとして維持される
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

#[test]
fn root_index_contains_bounded_contents_preview() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("backbone")).unwrap();
    fs::create_dir_all(target.join("scenario")).unwrap();
    fs::write(target.join("backbone/world.md"), "# World\n\n## Nations\n").unwrap();
    fs::write(target.join("scenario/route.md"), "# Route\n\n## Politics\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let idx = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(idx.contains("## Contents Preview"));
    assert!(idx.contains("- [backbone](fragments/backbone/_index.md) — 1 notes, 1 fragments"));
    assert!(idx.contains("- [scenario](fragments/scenario/_index.md) — 1 notes, 1 fragments"));
    assert!(idx.contains("### Headings Preview"));
    assert!(idx.contains("World"));
    assert!(idx.contains("Nations"));
    assert!(
        !idx.contains("(fragments/backbone/world/index.md)"),
        "root preview must not become a full note listing: {idx}"
    );
}

#[test]
fn fragments_index_shows_directory_density_and_examples() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("backbone")).unwrap();
    fs::write(target.join("backbone/world.md"), "# World\n\n## Nations\n").unwrap();
    fs::write(target.join("backbone/chronology.md"), "# Chronology\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let root_idx = fs::read_to_string(output.join("fragments/_index.md")).unwrap();
    assert!(root_idx.contains("- [backbone](backbone/_index.md) — 2 notes, 1 fragments"));
    assert!(root_idx.contains("  - Chronology"));
    assert!(root_idx.contains("  - World"));
}

#[test]
fn relative_markdown_links_resolve_to_generated_pages() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("a.md"),
        "# A\n\nSee [B](b.md) and [B Design](b.md#Design).\n",
    )
    .unwrap();
    fs::write(target.join("b.md"), "# B\n\n## Design\n\nDetails.\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let a = fs::read_to_string(output.join("fragments/a/index.md")).unwrap();
    assert!(
        a.contains("[B](../b/index.md)"),
        "entry link was not rewritten: {a}"
    );
    assert!(
        a.contains("[B Design](../b/design.md)"),
        "heading link was not rewritten to the fragment page: {a}"
    );
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

/// FR-11: frontmatter related は wikilink ターゲット指定でも入口ページに列挙される
#[test]
fn frontmatter_related_resolves_wikilink_targets() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.md"), "---\nrelated: [b]\n---\n# A").unwrap();
    fs::write(target.join("b.md"), "# B").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let a = fs::read_to_string(output.join("fragments/a/index.md")).unwrap();
    assert!(a.contains("## Related"));
    assert!(a.contains("- [B](../b/index.md)"));
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
    assert!(alpha.starts_with("---\nmd_wiki:\n"));
    assert!(
        alpha.contains("> Parent: [N](index.md) · Next: [Bravo](bravo.md)\n---\n\n"),
        "metadata の後にナビ行と水平線が続くこと: {alpha}"
    );
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

/// AC-12: ルート index.md のサマリに各カウントが出る
#[test]
fn root_index_summary_counts() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    // ノート 2 枚・h2 は合計 3 個
    fs::write(
        target.join("a.md"),
        "# A\n\n## Alpha\n\na\n\n## Bravo\n\nb\n",
    )
    .unwrap();
    fs::write(target.join("b.md"), "# B\n\n## Charlie\n\nc\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let idx = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(
        idx.contains("- Notes: 2"),
        "Notes カウントが出ること: {idx}"
    );
    assert!(
        idx.contains("- Fragments: 3"),
        "Fragments カウント（h2 断片）が出ること: {idx}"
    );
    assert!(idx.contains("- Tags:"));
    assert!(idx.contains("- Unresolved links:"));
}

/// AC-17: h3 再分割の殻ページと子断片ページ
#[test]
fn h3_resplit_produces_shell_and_children() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    let mut body = String::from("# N\n\n## Design\n\n");
    body.push_str("### Alpha\n");
    for i in 0..160 {
        body.push_str(&format!("a{i}\n"));
    }
    body.push_str("### Bravo\n");
    for i in 0..160 {
        body.push_str(&format!("b{i}\n"));
    }
    fs::write(target.join("n.md"), body).unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    // 殻ページと子断片
    assert!(output.join("fragments/n/design/index.md").exists());
    assert!(output.join("fragments/n/design/alpha.md").exists());
    assert!(output.join("fragments/n/design/bravo.md").exists());
    // 通常 h2 の `fragments/n/design.md` は存在しない
    assert!(!output.join("fragments/n/design.md").exists());

    // 入口ページの Fragments は殻 index.md を指す
    let entry = fs::read_to_string(output.join("fragments/n/index.md")).unwrap();
    assert!(
        entry.contains("design/index.md"),
        "入口は殻 index.md を指す: {entry}"
    );

    // 殻ページに Parent のみ、子断片一覧、Prev/Next 無し
    let shell = fs::read_to_string(output.join("fragments/n/design/index.md")).unwrap();
    assert!(shell.contains("Parent:"));
    assert!(!shell.contains("Prev:"));
    assert!(!shell.contains("Next:"));
    assert!(shell.contains("## Fragments"));
    assert!(shell.contains("alpha.md"));
    assert!(shell.contains("bravo.md"));

    // 子断片に Parent = 殻ページ、Prev/Next
    let bravo = fs::read_to_string(output.join("fragments/n/design/bravo.md")).unwrap();
    assert!(bravo.contains("Parent: [Design](index.md)"));
    assert!(bravo.contains("Prev: [Alpha](alpha.md)"));
    assert!(!bravo.contains("Next:"));
}

/// AC-11: Backlinks が参照元ページ出力相対パス昇順で並ぶ
#[test]
fn backlinks_ordered_by_source_path() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("a")).unwrap();
    fs::create_dir_all(target.join("b")).unwrap();
    fs::write(target.join("a/one.md"), "see [[target]]").unwrap();
    fs::write(target.join("b/two.md"), "see [[target]]").unwrap();
    fs::write(target.join("target.md"), "# Target").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let t = fs::read_to_string(output.join("fragments/target/index.md")).unwrap();
    let a_idx = t.find("a/one").unwrap();
    let b_idx = t.find("b/two").unwrap();
    assert!(a_idx < b_idx, "a/one が b/two より先に来る: {t}");
}

/// AC-21: fragments 配下の各ディレクトリに `_index.md` が生成される
#[test]
fn site_index_generated_for_each_section() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(target.join("docs/auth")).unwrap();
    fs::write(target.join("root.md"), "# Root\n\nx\n").unwrap();
    fs::write(target.join("docs/auth/session.md"), "# Session\n\ny\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let root_idx = fs::read_to_string(output.join("fragments/_index.md")).unwrap();
    let docs_idx = fs::read_to_string(output.join("fragments/docs/_index.md")).unwrap();
    let auth_idx = fs::read_to_string(output.join("fragments/docs/auth/_index.md")).unwrap();

    assert!(root_idx.contains("# fragments"));
    assert!(root_idx.contains("- [Root](root/index.md)"));
    assert!(root_idx.contains("- [docs](docs/_index.md) — 1 ノート"));
    assert!(root_idx.contains("- Notes: 2 (recursive)"));

    assert!(docs_idx.contains("- [auth](auth/_index.md) — 1 ノート"));
    assert!(auth_idx.contains("- [Session](session/index.md)"));

    // 入口 index.md と _index.md は共存する（衝突しない）
    assert!(output.join("fragments/root/index.md").exists());
    assert!(output.join("fragments/docs/auth/session/index.md").exists());
}

/// AC-22: ルート index.md には全ノート列挙が含まれず、fragments/_index.md への導線のみが並ぶ
#[test]
fn root_index_is_reduced_to_sitemap() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("a.md"), "# A\n\nx\n").unwrap();
    fs::write(target.join("b.md"), "# B\n\ny\n").unwrap();
    fs::write(target.join("c.md"), "# C\n\nz\n").unwrap();

    let output = tmp.path().join("out");
    generate_dir(&target, &output, "project", true);

    let idx = fs::read_to_string(output.join("index.md")).unwrap();
    assert!(idx.contains("- [Notes](fragments/_index.md)"));
    assert!(idx.contains("- [Tags](tags/index.md)"));
    assert!(idx.contains("- [Headings](headings/index.md)"));
    assert!(idx.contains("- [Links](links/index.md)"));

    // 全ノート列挙は含まれない（v1.1 時点での個別ノートリンク形式を拒否）
    assert!(
        !idx.contains("(fragments/a/index.md)"),
        "ルート index にノート直リンクが含まれている: {idx}"
    );
    assert!(
        !idx.contains("(fragments/b/index.md)"),
        "ルート index にノート直リンクが含まれている: {idx}"
    );
    assert!(
        !idx.contains("(fragments/c/index.md)"),
        "ルート index にノート直リンクが含まれている: {idx}"
    );
}

/// AC-23: 40,000 文字を超えるページで warn ログにパスと文字数が出る
#[test]
fn oversized_page_emits_warn_log() {
    use std::sync::{Arc, Mutex};
    use tracing::field::{Field, Visit};
    use tracing::{Event, Metadata, Subscriber, subscriber::with_default};

    #[derive(Default)]
    struct Capture {
        events: Mutex<Vec<(tracing::Level, String)>>,
    }

    struct LineVisitor(String);
    impl Visit for LineVisitor {
        fn record_debug(&mut self, f: &Field, v: &dyn std::fmt::Debug) {
            use std::fmt::Write;
            let _ = write!(self.0, "{}={:?} ", f.name(), v);
        }
        fn record_str(&mut self, f: &Field, v: &str) {
            use std::fmt::Write;
            let _ = write!(self.0, "{}={} ", f.name(), v);
        }
        fn record_u64(&mut self, f: &Field, v: u64) {
            use std::fmt::Write;
            let _ = write!(self.0, "{}={} ", f.name(), v);
        }
        fn record_i64(&mut self, f: &Field, v: i64) {
            use std::fmt::Write;
            let _ = write!(self.0, "{}={} ", f.name(), v);
        }
    }

    impl Subscriber for Capture {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, event: &Event<'_>) {
            let mut v = LineVisitor(String::new());
            event.record(&mut v);
            self.events
                .lock()
                .unwrap()
                .push((*event.metadata().level(), v.0));
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }

    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("project");
    fs::create_dir_all(&target).unwrap();
    let body_line = "あいうえおかきくけこ".repeat(100);
    let mut body = String::from("# Huge\n\n");
    for _ in 0..45 {
        body.push_str(&body_line);
        body.push('\n');
    }
    fs::write(target.join("huge.md"), &body).unwrap();

    struct Wrap(Arc<Capture>);
    impl Subscriber for Wrap {
        fn enabled(&self, m: &Metadata<'_>) -> bool {
            self.0.enabled(m)
        }
        fn new_span(&self, a: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            self.0.new_span(a)
        }
        fn record(&self, id: &tracing::span::Id, r: &tracing::span::Record<'_>) {
            self.0.record(id, r)
        }
        fn record_follows_from(&self, a: &tracing::span::Id, b: &tracing::span::Id) {
            self.0.record_follows_from(a, b)
        }
        fn event(&self, e: &Event<'_>) {
            self.0.event(e)
        }
        fn enter(&self, id: &tracing::span::Id) {
            self.0.enter(id)
        }
        fn exit(&self, id: &tracing::span::Id) {
            self.0.exit(id)
        }
    }

    let capture = Arc::new(Capture::default());
    let output = tmp.path().join("out");
    with_default(Wrap(Arc::clone(&capture)), || {
        generate_dir(&target, &output, "project", true);
    });

    assert!(output.join("fragments/huge/index.md").exists());

    let events = capture.events.lock().unwrap();
    let hit = events.iter().any(|(lvl, msg)| {
        *lvl == tracing::Level::WARN
            && msg.contains("fragments/huge/index.md")
            && msg.contains("chars=")
    });
    assert!(
        hit,
        "warn ログにパスと文字数が含まれるべき: {:?}",
        events.iter().collect::<Vec<_>>()
    );
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
