//! CLI 外周の挙動を検証する統合テスト。

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn bin_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_md-wiki"))
}

fn run(args: &[&std::ffi::OsStr]) -> std::process::Output {
    let output = Command::new(bin_path()).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "md-wiki failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

#[test]
fn cli_help_runs() {
    let output = Command::new(bin_path()).arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("md-wiki"),
        "--help には md-wiki の記述が必要"
    );
}

#[test]
fn single_file_input_generates_wiki() {
    let tmp = TempDir::new().unwrap();
    let note = tmp.path().join("memo.md");
    fs::write(&note, "# Memo\n\nsome body\n\n## Detail\n\nmore").unwrap();

    let out = tmp.path().join("wiki");
    run(&[note.as_os_str(), "--out".as_ref(), out.as_os_str()]);

    assert!(out.join("index.md").exists());
    assert!(out.join("fragments/memo/index.md").exists());
    assert!(out.join("fragments/memo/detail.md").exists());
}

#[test]
fn directory_recursive_includes_nested_and_excludes_node_modules() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(input.join("deep")).unwrap();
    fs::create_dir_all(input.join("node_modules")).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();
    fs::write(input.join("deep/b.md"), "# B").unwrap();
    fs::write(input.join("node_modules/bad.md"), "# Bad").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    assert!(out.join("fragments/a/index.md").exists());
    assert!(out.join("fragments/deep/b/index.md").exists());
    assert!(!out.join("fragments/node_modules").exists());
}

#[test]
fn rejects_non_md_file_input() {
    let tmp = TempDir::new().unwrap();
    let bad = tmp.path().join("not-markdown.txt");
    fs::write(&bad, "hi").unwrap();

    let out = tmp.path().join("wiki");
    let output = Command::new(bin_path())
        .args([bad.as_os_str(), "--out".as_ref(), out.as_os_str()])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn refuses_to_clean_non_md_wiki_directory() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();

    // 出力先に無関係なファイルが入っているケース
    let out = tmp.path().join("out");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("user-file.txt"), "dont delete me").unwrap();

    let output = Command::new(bin_path())
        .args([
            input.as_os_str(),
            "--recursive".as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "md-wiki 以外のディレクトリを掃除しようとしたらエラー終了すべき"
    );
    // ユーザのファイルが残っていることを確認
    assert!(out.join("user-file.txt").exists());
}

#[test]
fn wiki_false_frontmatter_excludes_note() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("keep.md"), "# Keep").unwrap();
    fs::write(input.join("skip.md"), "---\nwiki: false\n---\n# Skip").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    assert!(out.join("fragments/keep/index.md").exists());
    assert!(!out.join("fragments/skip/index.md").exists());
}
