use std::fmt::Write;

use crate::extract::{ManifestKind, TechStack};

pub fn render_development(stack: &TechStack) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Development");
    s.push('\n');
    let _ = writeln!(
        &mut s,
        "以下はマニフェスト検出から推定したビルド/テストコマンド候補。実際の手順はプロジェクトの README やドキュメントも併せて確認すること。"
    );
    s.push('\n');

    if stack.manifests.is_empty() {
        let _ = writeln!(&mut s, "_(no manifests detected)_");
        return s;
    }

    for m in &stack.manifests {
        let _ = writeln!(&mut s, "## `{}` ({})", m.file.display(), m.kind.label());
        s.push('\n');
        let cmds = commands_for(m.kind);
        for (label, cmd) in cmds {
            let _ = writeln!(&mut s, "- {label}: `{cmd}`");
        }
        s.push('\n');
    }
    s
}

fn commands_for(kind: ManifestKind) -> &'static [(&'static str, &'static str)] {
    match kind {
        ManifestKind::CargoToml => &[
            ("build", "cargo build"),
            ("test", "cargo test"),
            ("lint", "cargo clippy -- -D warnings"),
            ("format", "cargo fmt"),
        ],
        ManifestKind::PackageJson => &[("install", "npm install"), ("test", "npm test")],
        ManifestKind::PyprojectToml => &[("install", "pip install -e ."), ("test", "pytest")],
        ManifestKind::RequirementsTxt => &[
            ("install", "pip install -r requirements.txt"),
            ("test", "pytest"),
        ],
        ManifestKind::GoMod => &[("build", "go build ./..."), ("test", "go test ./...")],
        ManifestKind::PomXml => &[("build", "mvn package"), ("test", "mvn test")],
        ManifestKind::Gemfile => &[("install", "bundle install"), ("test", "bundle exec rspec")],
    }
}
