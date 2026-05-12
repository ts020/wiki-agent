//! CLI 外周の挙動を検証する統合テスト。

use std::fs;
use std::path::{Path, PathBuf};
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

fn run_fail(args: &[&std::ffi::OsStr]) -> std::process::Output {
    let output = Command::new(bin_path()).args(args).output().unwrap();
    assert!(
        !output.status.success(),
        "md-wiki unexpectedly succeeded: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn snapshot_dir(path: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    if !path.exists() {
        return out;
    }
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(path).unwrap().to_path_buf();
            out.insert(rel, fs::read(entry.path()).unwrap());
        }
    }
    out
}

fn large_markdown_body() -> String {
    let line = format!("{}\n", "large markdown content ".repeat(40));
    let mut body = String::from("# Huge\n\n");
    while body.len() <= 1024 * 1024 + 4096 {
        body.push_str(&line);
    }
    body
}

fn shell_markdown_body() -> String {
    let mut body = String::from("# N\n\n## Design\n\n");
    for idx in 0..301 {
        body.push_str(&format!("line {idx}\n"));
    }
    body.push_str("\n### Alpha\n\nalpha\n\n### Beta\n\nbeta\n");
    body
}

fn output_lock_path(output: &Path) -> PathBuf {
    let abs = if let Ok(path) = output.canonicalize() {
        path
    } else if let Some(parent) = output.parent()
        && let Ok(parent) = parent.canonicalize()
        && let Some(name) = output.file_name()
    {
        parent.join(name)
    } else {
        std::path::absolute(output).unwrap_or_else(|_| output.to_path_buf())
    };
    let hash = stable_hash(abs.to_string_lossy().replace('\\', "/").as_bytes());
    std::env::temp_dir()
        .join("md-wiki-locks")
        .join(format!("{hash}.lock"))
}

fn stable_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[test]
fn cli_help_runs() {
    let output = Command::new(bin_path()).arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("md-wiki"),
        "--help には md-wiki の記述が必要"
    );
    assert!(stdout.contains("init"));
    assert!(stdout.contains("add"));
}

#[test]
fn old_positional_cli_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let note = tmp.path().join("memo.md");
    fs::write(&note, "# Memo").unwrap();

    run_fail(&[note.as_os_str()]);
}

#[test]
fn init_single_file_input_generates_wiki_and_manifest() {
    let tmp = TempDir::new().unwrap();
    let note = tmp.path().join("memo.md");
    fs::write(&note, "# Memo\n\nsome body\n\n## Detail\n\nmore").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        note.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    assert!(out.join("index.md").exists());
    assert!(out.join("fragments/memo/index.md").exists());
    assert!(out.join("fragments/memo/detail.md").exists());
    assert!(out.join(".md-wiki/manifest.json").exists());
    assert!(!output_lock_path(&out).exists());
}

#[test]
fn init_manifest_records_input_sources_and_generated_files() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join(".md-wiki/manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["input_kind"], "directory");
    assert_eq!(manifest["recursive"], true);
    assert_eq!(
        manifest["input_root"].as_str().unwrap(),
        input
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    );
    assert!(manifest["source_hashes"]["a.md"].as_str().is_some());
    assert!(
        manifest["generated_file_hashes"]["index.md"]
            .as_str()
            .is_some()
    );
    assert!(
        manifest["generated_file_hashes"]["fragments/a/index.md"]
            .as_str()
            .is_some()
    );
    assert!(manifest["generated_file_hashes"][".md-wiki/manifest.json"].is_null());
}

#[test]
fn init_directory_recursive_includes_nested_and_excludes_node_modules() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(input.join("deep")).unwrap();
    fs::create_dir_all(input.join("node_modules")).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();
    fs::write(input.join("deep/b.md"), "# B").unwrap();
    fs::write(input.join("node_modules/bad.md"), "# Bad").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
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
    run_fail(&[
        "init".as_ref(),
        bad.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
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
            "init".as_ref(),
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
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    assert!(out.join("fragments/keep/index.md").exists());
    assert!(!out.join("fragments/skip/index.md").exists());
}

#[test]
fn add_updates_indexes_and_matches_fresh_init() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(
        input.join("a.md"),
        "---\ntitle: A\ntags: [x]\n---\nlinks [[b]]",
    )
    .unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    fs::write(input.join("b.md"), "---\ntitle: B\ntags: [x]\n---\n# B").unwrap();
    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);

    let a = fs::read_to_string(out.join("fragments/a/index.md")).unwrap();
    assert!(a.contains("[b](../b/index.md)"));
    let index = fs::read_to_string(out.join("index.md")).unwrap();
    assert!(!index.contains("## Unresolved"));
    assert!(out.join("agent/pages/index.md").exists());

    let fresh = tmp.path().join("fresh");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        fresh.as_os_str(),
    ]);
    assert_eq!(snapshot_dir(&out), snapshot_dir(&fresh));
}

#[test]
fn add_removes_pages_for_deleted_sources() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();
    fs::write(input.join("b.md"), "# B").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(out.join("fragments/b/index.md").exists());

    fs::remove_file(input.join("b.md")).unwrap();
    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);

    assert!(!out.join("fragments/b/index.md").exists());
}

#[test]
fn add_single_file_source_deletion_removes_managed_pages() {
    let tmp = TempDir::new().unwrap();
    let note = tmp.path().join("memo.md");
    fs::write(&note, "# Memo\n\n## Detail\n\nbody").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        note.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(out.join("fragments/memo/index.md").exists());
    assert!(out.join("fragments/memo/detail.md").exists());

    fs::remove_file(&note).unwrap();
    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);

    assert!(out.join("index.md").exists());
    assert!(!out.join("fragments/memo/index.md").exists());
    assert!(!out.join("fragments/memo/detail.md").exists());
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join(".md-wiki/manifest.json")).unwrap()).unwrap();
    assert!(manifest["source_hashes"].as_object().unwrap().is_empty());
}

#[test]
fn add_handles_large_markdown_add_delete_and_matches_fresh_init() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    fs::write(input.join("huge.md"), large_markdown_body()).unwrap();
    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);

    assert!(out.join("fragments/huge/index.md").exists());
    assert!(out.join("fragments/huge/part-001.md").exists());
    let huge_entry = fs::read_to_string(out.join("fragments/huge/index.md")).unwrap();
    assert!(huge_entry.contains("source: \"huge.md\""));

    let fresh = tmp.path().join("fresh");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        fresh.as_os_str(),
    ]);
    assert_eq!(snapshot_dir(&out), snapshot_dir(&fresh));

    fs::remove_file(input.join("huge.md")).unwrap();
    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);

    assert!(!out.join("fragments/huge/index.md").exists());
    let fresh_after_delete = tmp.path().join("fresh-after-delete");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        fresh_after_delete.as_os_str(),
    ]);
    assert_eq!(snapshot_dir(&out), snapshot_dir(&fresh_after_delete));
}

#[test]
fn add_handles_fragment_file_to_shell_directory_shape_change() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("n.md"), "# N\n\n## Design\n\nshort\n").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(out.join("fragments/n/design.md").exists());

    fs::write(input.join("n.md"), shell_markdown_body()).unwrap();
    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);

    assert!(!out.join("fragments/n/design.md").exists());
    assert!(out.join("fragments/n/design/index.md").exists());
    assert!(out.join("fragments/n/design/alpha.md").exists());
    assert!(out.join("fragments/n/design/beta.md").exists());

    let fresh = tmp.path().join("fresh");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        fresh.as_os_str(),
    ]);
    assert_eq!(snapshot_dir(&out), snapshot_dir(&fresh));
}

#[cfg(unix)]
#[test]
fn add_excludes_output_when_input_root_is_symlinked() {
    let tmp = TempDir::new().unwrap();
    let real = tmp.path().join("real");
    let link = tmp.path().join("link");
    fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    fs::write(real.join("a.md"), "# A").unwrap();

    let out = link.join("md-wiki");
    run(&[
        "init".as_ref(),
        link.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    fs::write(real.join("b.md"), "# B").unwrap();
    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);

    assert!(out.join("fragments/a/index.md").exists());
    assert!(out.join("fragments/b/index.md").exists());
    assert!(
        !out.join("fragments/md-wiki").exists(),
        "add must not ingest its generated output through the canonical input root"
    );

    let incremental = snapshot_dir(&out);
    fs::remove_dir_all(&out).unwrap();
    run(&[
        "init".as_ref(),
        real.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(incremental, snapshot_dir(&out));
}

#[test]
fn add_does_not_rewrite_unchanged_generated_pages() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let unchanged = out.join("fragments/a/index.md");
    let mut perms = fs::metadata(&unchanged).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&unchanged, perms.clone()).unwrap();

    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);

    perms.set_readonly(false);
    fs::set_permissions(&unchanged, perms).unwrap();
}

#[test]
fn add_requires_manifest_and_rejects_invalid_paths_or_unmanaged_collisions() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();
    let out = tmp.path().join("wiki");

    run_fail(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);

    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let outside = tmp.path().join("outside.md");
    fs::write(&outside, "# Outside").unwrap();
    run_fail(&[
        "add".as_ref(),
        outside.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    fs::write(input.join("c.md"), "# C").unwrap();
    fs::create_dir_all(out.join("fragments/c")).unwrap();
    fs::write(out.join("fragments/c/index.md"), "# unmanaged").unwrap();
    let before_failed_collision = snapshot_dir(&out);
    run_fail(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);
    assert_eq!(snapshot_dir(&out), before_failed_collision);
}

#[test]
fn add_rejects_unmanaged_parent_file_before_mutating_output() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A\n\n[[c]]").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let a_before = fs::read_to_string(out.join("fragments/a/index.md")).unwrap();
    fs::write(input.join("c.md"), "# C").unwrap();
    fs::write(out.join("fragments/c"), "unmanaged parent file").unwrap();
    let before_failed_collision = snapshot_dir(&out);

    let output = run_fail(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("parent path is not a directory"),
        "stderr should explain parent-path collision"
    );
    assert_eq!(snapshot_dir(&out), before_failed_collision);
    assert_eq!(
        fs::read_to_string(out.join("fragments/a/index.md")).unwrap(),
        a_before
    );
}

#[test]
fn add_rejects_manifest_generated_paths_outside_output() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let victim = tmp.path().join("victim.txt");
    fs::write(&victim, "do not delete").unwrap();
    let manifest_path = out.join(".md-wiki/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["generated_file_hashes"]
        .as_object_mut()
        .unwrap()
        .insert(
            "../victim.txt".into(),
            serde_json::Value::String("bad".into()),
        );
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let output = run_fail(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("invalid generated output path"),
        "stderr should explain manifest path validation failure"
    );
    assert_eq!(fs::read_to_string(&victim).unwrap(), "do not delete");
}

#[cfg(unix)]
#[test]
fn add_rejects_symlink_at_managed_output_path() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    let target = tmp.path().join("outside.txt");
    fs::write(&target, "outside").unwrap();
    let managed = out.join("fragments/a/index.md");
    fs::remove_file(&managed).unwrap();
    std::os::unix::fs::symlink(&target, &managed).unwrap();
    fs::write(input.join("a.md"), "# A changed").unwrap();

    let output = run_fail(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("symlink output path"),
        "stderr should explain symlink refusal"
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "outside");
}

#[test]
fn add_rejects_corrupt_manifest() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();

    let out = tmp.path().join("wiki");
    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);

    fs::write(out.join(".md-wiki/manifest.json"), "{not json").unwrap();
    run_fail(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);
}

#[test]
fn init_and_add_reject_when_output_lock_exists() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("src");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("a.md"), "# A").unwrap();

    let out = tmp.path().join("wiki");
    let lock = output_lock_path(&out);
    fs::create_dir_all(lock.parent().unwrap()).unwrap();
    fs::write(&lock, "pid=test").unwrap();
    let output = run_fail(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("output is locked"),
        "stderr should explain lock failure"
    );
    fs::remove_file(&lock).unwrap();

    run(&[
        "init".as_ref(),
        input.as_os_str(),
        "--recursive".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!lock.exists());

    fs::write(&lock, "pid=test").unwrap();
    let output = run_fail(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("output is locked"),
        "stderr should explain lock failure"
    );
    fs::remove_file(&lock).unwrap();

    run(&["add".as_ref(), "--out".as_ref(), out.as_os_str()]);
    assert!(!lock.exists());
}
