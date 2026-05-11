#[test]
fn readme_uses_current_fragments_output_structure() {
    let readme = include_str!("../README.md");
    assert!(readme.contains("├── fragments/"));
    assert!(!readme.contains("├── notes/"));
    assert!(!readme.contains("`notes/`"));
}

#[test]
fn readme_documents_qwen3_reading_order() {
    let readme = include_str!("../README.md");
    assert!(readme.contains("## Qwen3 推奨読み順"));
    assert!(readme.contains("agent/pages/index.md"));
    assert!(readme.contains("agent/terms/index.md"));
}

#[test]
fn cargo_package_metadata_is_ready_for_public_distribution() {
    let manifest: toml::Value = include_str!("../Cargo.toml").parse().unwrap();
    let package = manifest.get("package").unwrap();

    assert_eq!(
        package.get("name").and_then(|v| v.as_str()),
        Some("md-wiki-cli")
    );
    assert_eq!(package.get("license").and_then(|v| v.as_str()), Some("MIT"));
    assert!(
        package
            .get("description")
            .and_then(|v| v.as_str())
            .is_some()
    );
    assert!(package.get("repository").and_then(|v| v.as_str()).is_some());
    assert!(package.get("homepage").and_then(|v| v.as_str()).is_some());
    assert!(
        package
            .get("rust-version")
            .and_then(|v| v.as_str())
            .is_some()
    );
    assert!(
        package
            .get("keywords")
            .and_then(|v| v.as_array())
            .is_some_and(|items| !items.is_empty())
    );
    assert!(
        package
            .get("categories")
            .and_then(|v| v.as_array())
            .is_some_and(|items| !items.is_empty())
    );

    assert_eq!(
        manifest
            .get("lib")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str()),
        Some("md_wiki")
    );
    let bins = manifest
        .get("bin")
        .and_then(|v| v.as_array())
        .expect("manifest should declare CLI binary");
    assert!(bins.iter().any(|bin| {
        bin.get("name").and_then(|v| v.as_str()) == Some("md-wiki")
            && bin.get("path").and_then(|v| v.as_str()) == Some("src/main.rs")
    }));

    let excludes = package
        .get("exclude")
        .and_then(|v| v.as_array())
        .expect("package.exclude should exclude local agent files");
    for pattern in [
        ".github/**",
        ".context/**",
        ".agents/**",
        ".claude/**",
        "AGENTS.md",
        "CLAUDE.md",
        "Cargo.toml.orig",
    ] {
        assert!(
            excludes.iter().any(|item| item.as_str() == Some(pattern)),
            "missing package.exclude pattern: {pattern}"
        );
    }
}

#[test]
fn readme_has_public_user_onboarding() {
    let readme = include_str!("../README.md");

    for expected in [
        "curl -fsSL https://raw.githubusercontent.com/ts020/wiki-agent/main/install.sh | sh",
        "MD_WIKI_INSTALL_DIR",
        "MD_WIKI_VERSION",
        "cargo install --path .",
        "cargo install md-wiki-cli",
        "binary name is still `md-wiki`",
        "## 最小例",
        "wiki: false",
        "fragment: false",
        "## 安全性と制限",
        "入力側の `.md` は変更しない",
        "## ライセンス",
    ] {
        assert!(readme.contains(expected), "README missing: {expected}");
    }
}

#[test]
fn release_installer_is_documented_and_wired_to_release_assets() {
    let install = include_str!("../install.sh");
    let workflow = include_str!("../.github/workflows/release.yml");
    let release_notes = include_str!("../docs/releases/v0.1.0.md");

    for expected in [
        "MD_WIKI_REPO",
        "MD_WIKI_VERSION",
        "MD_WIKI_INSTALL_DIR",
        "releases/latest/download",
        "checksums.txt",
        "x86_64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        assert!(install.contains(expected), "install.sh missing: {expected}");
    }

    for expected in [
        "push:",
        "tags:",
        "md-wiki-${{ matrix.target }}.tar.gz",
        "sha256sum md-wiki-*.tar.gz > checksums.txt",
        "gh release create",
    ] {
        assert!(
            workflow.contains(expected),
            "release workflow missing: {expected}"
        );
    }

    assert!(release_notes.contains("curl -fsSL"));
}

#[test]
fn requirements_match_current_large_markdown_behavior() {
    let input = include_str!("../docs/要件定義/07-入力.md");
    let errors = include_str!("../docs/要件定義/12-エラー処理.md");
    let decided = include_str!("../docs/要件定義/15-確定済み仕様.md");

    let all = [input, errors, decided].join("\n");
    assert!(!all.contains("1 MiB 超はスキップ"));
    assert!(!all.contains("ファイル 1 MiB 超・バイナリ・symlink はスキップ"));
    assert!(all.contains("1 MiB 超"));
    assert!(all.contains("large path"));
    assert!(all.contains("200 MiB"));
}
