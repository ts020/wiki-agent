use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::Deserialize;

use crate::scan::ScannedFile;

#[derive(Debug, Default)]
pub struct TechStack {
    pub languages: BTreeSet<String>,
    pub manifests: Vec<Manifest>,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub file: PathBuf,
    pub kind: ManifestKind,
    pub project_name: Option<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestKind {
    CargoToml,
    PackageJson,
    PyprojectToml,
    RequirementsTxt,
    GoMod,
    PomXml,
    Gemfile,
}

impl ManifestKind {
    pub fn language(self) -> &'static str {
        match self {
            Self::CargoToml => "Rust",
            Self::PackageJson => "JavaScript/TypeScript",
            Self::PyprojectToml | Self::RequirementsTxt => "Python",
            Self::GoMod => "Go",
            Self::PomXml => "Java",
            Self::Gemfile => "Ruby",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CargoToml => "Cargo.toml",
            Self::PackageJson => "package.json",
            Self::PyprojectToml => "pyproject.toml",
            Self::RequirementsTxt => "requirements.txt",
            Self::GoMod => "go.mod",
            Self::PomXml => "pom.xml",
            Self::Gemfile => "Gemfile",
        }
    }
}

pub fn detect_tech_stack(scanned: &[ScannedFile], target_root: &Path) -> TechStack {
    let mut stack = TechStack::default();
    for f in scanned {
        let Some(kind) = classify(&f.relative_path) else {
            continue;
        };
        let abs = target_root.join(&f.relative_path);
        if let Some(m) = parse_manifest(kind, &abs, &f.relative_path) {
            stack.languages.insert(m.kind.language().to_string());
            stack.manifests.push(m);
        }
    }
    stack.manifests.sort_by(|a, b| a.file.cmp(&b.file));
    stack
}

fn classify(rel: &Path) -> Option<ManifestKind> {
    let name = rel.file_name()?.to_str()?;
    Some(match name {
        "Cargo.toml" => ManifestKind::CargoToml,
        "package.json" => ManifestKind::PackageJson,
        "pyproject.toml" => ManifestKind::PyprojectToml,
        "requirements.txt" => ManifestKind::RequirementsTxt,
        "go.mod" => ManifestKind::GoMod,
        "pom.xml" => ManifestKind::PomXml,
        "Gemfile" => ManifestKind::Gemfile,
        _ => return None,
    })
}

fn parse_manifest(kind: ManifestKind, abs: &Path, rel: &Path) -> Option<Manifest> {
    let content = match fs::read_to_string(abs) {
        Ok(c) => c,
        Err(err) => {
            tracing::warn!(path = %rel.display(), error = %err, "failed to read manifest");
            return None;
        }
    };
    let (name, deps) = match kind {
        ManifestKind::CargoToml => parse_cargo(&content),
        ManifestKind::PackageJson => parse_package_json(&content),
        ManifestKind::PyprojectToml => parse_pyproject(&content),
        ManifestKind::RequirementsTxt => parse_requirements(&content),
        ManifestKind::GoMod => parse_go_mod(&content),
        ManifestKind::PomXml => parse_pom_xml(&content),
        ManifestKind::Gemfile => parse_gemfile(&content),
    };
    Some(Manifest {
        file: rel.to_path_buf(),
        kind,
        project_name: name,
        dependencies: deps,
    })
}

#[derive(Deserialize)]
struct CargoToml {
    package: Option<CargoPackage>,
    #[serde(default)]
    dependencies: toml::value::Table,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: Option<String>,
}

fn parse_cargo(content: &str) -> (Option<String>, Vec<String>) {
    match toml::from_str::<CargoToml>(content) {
        Ok(c) => {
            let name = c.package.and_then(|p| p.name);
            let mut deps: Vec<String> = c.dependencies.keys().cloned().collect();
            deps.sort();
            (name, deps)
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to parse Cargo.toml");
            (None, Vec::new())
        }
    }
}

#[derive(Deserialize)]
struct PackageJson {
    name: Option<String>,
    #[serde(default)]
    dependencies: serde_json::Map<String, serde_json::Value>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: serde_json::Map<String, serde_json::Value>,
}

fn parse_package_json(content: &str) -> (Option<String>, Vec<String>) {
    match serde_json::from_str::<PackageJson>(content) {
        Ok(p) => {
            let mut deps: Vec<String> = p
                .dependencies
                .keys()
                .chain(p.dev_dependencies.keys())
                .cloned()
                .collect();
            deps.sort();
            deps.dedup();
            (p.name, deps)
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to parse package.json");
            (None, Vec::new())
        }
    }
}

fn parse_pyproject(content: &str) -> (Option<String>, Vec<String>) {
    let parsed: toml::Value = match toml::from_str(content) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(error = %err, "failed to parse pyproject.toml");
            return (None, Vec::new());
        }
    };
    let name = parsed
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(String::from)
        .or_else(|| {
            parsed
                .get("tool")
                .and_then(|t| t.get("poetry"))
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .map(String::from)
        });
    let mut deps: Vec<String> = parsed
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(extract_pkg)
                .collect()
        })
        .unwrap_or_default();
    deps.sort();
    deps.dedup();
    deps.retain(|s| !s.is_empty());
    (name, deps)
}

fn extract_pkg(spec: &str) -> String {
    let stop = |c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.';
    spec.trim_start()
        .split(stop)
        .next()
        .unwrap_or("")
        .to_string()
}

fn parse_requirements(content: &str) -> (Option<String>, Vec<String>) {
    let mut deps: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('-'))
        .map(extract_pkg)
        .filter(|s| !s.is_empty())
        .collect();
    deps.sort();
    deps.dedup();
    (None, deps)
}

fn parse_go_mod(content: &str) -> (Option<String>, Vec<String>) {
    let mut name = None;
    let mut deps = Vec::new();
    let mut in_require = false;
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("module ") {
            name = Some(rest.trim().to_string());
            continue;
        }
        if line.starts_with("require (") {
            in_require = true;
            continue;
        }
        if in_require {
            if line == ")" {
                in_require = false;
                continue;
            }
            if let Some((pkg, _)) = line.split_once(' ') {
                deps.push(pkg.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("require ")
            && let Some((pkg, _)) = rest.trim().split_once(' ')
        {
            deps.push(pkg.to_string());
        }
    }
    deps.sort();
    deps.dedup();
    (name, deps)
}

fn parse_pom_xml(content: &str) -> (Option<String>, Vec<String>) {
    let artifact_re = Regex::new(r"<artifactId>\s*([^<]+?)\s*</artifactId>").unwrap();
    let dep_block_re = Regex::new(r"(?s)<dependency>.*?</dependency>").unwrap();
    // parent の artifactId を project 名と誤検出しないよう、<parent>..</parent> を除いてから検索
    let parent_re = Regex::new(r"(?s)<parent>.*?</parent>").unwrap();
    let stripped = parent_re.replace_all(content, "");
    let name = artifact_re
        .captures(&stripped)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string());
    let mut deps: Vec<String> = dep_block_re
        .find_iter(content)
        .filter_map(|m| {
            artifact_re
                .captures(m.as_str())
                .and_then(|c| c.get(1).map(|x| x.as_str().trim().to_string()))
        })
        .collect();
    deps.sort();
    deps.dedup();
    (name, deps)
}

fn parse_gemfile(content: &str) -> (Option<String>, Vec<String>) {
    let re = Regex::new(r#"(?m)^\s*gem\s+['"]([^'"]+)['"]"#).unwrap();
    let mut deps: Vec<String> = re
        .captures_iter(content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();
    deps.sort();
    deps.dedup();
    (None, deps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cargo_toml() {
        let src = r#"
            [package]
            name = "sample"
            [dependencies]
            serde = "1"
            tokio = { version = "1", features = ["full"] }
        "#;
        let (name, deps) = parse_cargo(src);
        assert_eq!(name.as_deref(), Some("sample"));
        assert_eq!(deps, vec!["serde", "tokio"]);
    }

    #[test]
    fn parses_package_json() {
        let src = r#"{
            "name": "app",
            "dependencies": { "react": "18", "lodash": "4" },
            "devDependencies": { "jest": "29" }
        }"#;
        let (name, deps) = parse_package_json(src);
        assert_eq!(name.as_deref(), Some("app"));
        assert_eq!(deps, vec!["jest", "lodash", "react"]);
    }

    #[test]
    fn parses_pyproject() {
        let src = r#"
            [project]
            name = "pkg"
            dependencies = ["requests>=2", "numpy"]
        "#;
        let (name, deps) = parse_pyproject(src);
        assert_eq!(name.as_deref(), Some("pkg"));
        assert_eq!(deps, vec!["numpy", "requests"]);
    }

    #[test]
    fn parses_requirements() {
        let src = "# comment\nrequests==2.0\nflask>=1\n\n-e .\n";
        let (_, deps) = parse_requirements(src);
        assert_eq!(deps, vec!["flask", "requests"]);
    }

    #[test]
    fn parses_go_mod() {
        let src = r#"module example.com/foo

go 1.21

require (
    github.com/a/b v1.0.0
    github.com/c/d v2.1.0
)

require github.com/e/f v0.1.0
"#;
        let (name, deps) = parse_go_mod(src);
        assert_eq!(name.as_deref(), Some("example.com/foo"));
        assert_eq!(
            deps,
            vec!["github.com/a/b", "github.com/c/d", "github.com/e/f"]
        );
    }

    #[test]
    fn parses_pom_xml() {
        let src = r#"<project>
<artifactId>my-app</artifactId>
<dependencies>
<dependency><groupId>g</groupId><artifactId>lib-a</artifactId></dependency>
<dependency><groupId>g</groupId><artifactId>lib-b</artifactId></dependency>
</dependencies>
</project>"#;
        let (name, deps) = parse_pom_xml(src);
        assert_eq!(name.as_deref(), Some("my-app"));
        assert_eq!(deps, vec!["lib-a", "lib-b"]);
    }

    #[test]
    fn pom_xml_project_name_ignores_parent() {
        let src = r#"<project>
<parent>
  <groupId>org</groupId>
  <artifactId>parent-project</artifactId>
  <version>1.0.0</version>
</parent>
<artifactId>child-app</artifactId>
<dependencies>
<dependency><artifactId>lib-x</artifactId></dependency>
</dependencies>
</project>"#;
        let (name, deps) = parse_pom_xml(src);
        assert_eq!(name.as_deref(), Some("child-app"));
        assert_eq!(deps, vec!["lib-x"]);
    }

    #[test]
    fn parses_gemfile() {
        let src = "source 'https://rubygems.org'\ngem 'rails'\ngem \"puma\"\n# gem 'comment'\n";
        let (_, deps) = parse_gemfile(src);
        assert_eq!(deps, vec!["puma", "rails"]);
    }
}
