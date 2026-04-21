use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

const FORBIDDEN_CHARS: &[char] = &['<', '>', ':', '"', '|', '?', '*', '\\'];

/// ファイル名に含められない文字を `_` に置換する。
pub fn sanitize_component(name: &str) -> String {
    name.chars()
        .map(|c| {
            if FORBIDDEN_CHARS.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// パス全体の各コンポーネントを sanitize する。
pub fn sanitize_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        if let Component::Normal(s) = comp {
            out.push(sanitize_component(s.to_string_lossy().as_ref()));
        }
    }
    out
}

/// ノートの入口ページ（ディレクトリ）を算出する。
/// - `README.md` → `fragments/README/`
/// - `docs/foo.md` → `fragments/docs/foo/`
pub fn entry_dir(source_file: &Path) -> PathBuf {
    let parent = source_file.parent().unwrap_or(Path::new(""));
    let stem = source_file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "note".to_string());
    let sanitized_parent = sanitize_path(parent);
    let mut dir = PathBuf::from("fragments");
    dir.push(sanitized_parent);
    dir.push(sanitize_component(&stem));
    dir
}

/// 入口ページの出力パス `fragments/<rel>/index.md`。
pub fn entry_index_path(source_file: &Path) -> PathBuf {
    entry_dir(source_file).join("index.md")
}

/// h2 通常断片ページの出力パス `<entry_dir>/<slug>.md`。
pub fn fragment_leaf_path(entry_dir: &Path, slug: &str) -> PathBuf {
    entry_dir.join(format!("{}.md", sanitize_component(slug)))
}

/// 殻ページ（h3 再分割時の h2 入口）の出力パス `<entry_dir>/<slug>/index.md`。
pub fn shell_index_path(entry_dir: &Path, slug: &str) -> PathBuf {
    entry_dir.join(sanitize_component(slug)).join("index.md")
}

/// h3 子断片ページの出力パス `<entry_dir>/<h2-slug>/<h3-slug>.md`。
pub fn h3_leaf_path(entry_dir: &Path, h2_slug: &str, h3_slug: &str) -> PathBuf {
    entry_dir
        .join(sanitize_component(h2_slug))
        .join(format!("{}.md", sanitize_component(h3_slug)))
}

/// 出力ルート相対の 2 パス間の相対リンク文字列を返す。
/// `from` がファイルの場合、その親ディレクトリからの相対として扱う。
pub fn relative_link(from: &Path, to: &Path) -> String {
    let from_parent = from.parent().unwrap_or(Path::new(""));
    let from_comps: Vec<_> = from_parent
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .collect();
    let to_comps: Vec<_> = to
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .collect();
    let mut common = 0;
    while common < from_comps.len()
        && common < to_comps.len()
        && from_comps[common] == to_comps[common]
    {
        common += 1;
    }
    let up = from_comps.len() - common;
    let mut parts: Vec<String> = Vec::new();
    for _ in 0..up {
        parts.push("..".to_string());
    }
    for c in &to_comps[common..] {
        if let Component::Normal(s) = c {
            parts.push(s.to_string_lossy().into_owned());
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// 同名出力パスの衝突を末尾 `-N` で解消する。
pub fn resolve_conflict(path: PathBuf, used: &mut HashSet<PathBuf>) -> PathBuf {
    if !used.contains(&path) {
        used.insert(path.clone());
        return path;
    }
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut n = 1u32;
    loop {
        let name = if ext.is_empty() {
            format!("{stem}-{n}")
        } else {
            format!("{stem}-{n}.{ext}")
        };
        let candidate = parent.join(&name);
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_forbidden_chars() {
        assert_eq!(sanitize_component("foo:bar"), "foo_bar");
        assert_eq!(sanitize_component("a|b*c"), "a_b_c");
        assert_eq!(sanitize_component("ok-name"), "ok-name");
    }

    #[test]
    fn entry_paths_for_root_file() {
        assert_eq!(
            entry_dir(Path::new("README.md")),
            PathBuf::from("fragments/README")
        );
        assert_eq!(
            entry_index_path(Path::new("README.md")),
            PathBuf::from("fragments/README/index.md")
        );
    }

    #[test]
    fn entry_paths_preserve_nested_structure() {
        assert_eq!(
            entry_index_path(Path::new("docs/a/b.md")),
            PathBuf::from("fragments/docs/a/b/index.md")
        );
    }

    #[test]
    fn entry_paths_sanitize_forbidden_chars() {
        assert_eq!(
            entry_index_path(Path::new("docs/a:b.md")),
            PathBuf::from("fragments/docs/a_b/index.md")
        );
    }

    #[test]
    fn fragment_leaf_and_shell_paths() {
        let dir = PathBuf::from("fragments/a");
        assert_eq!(
            fragment_leaf_path(&dir, "intro"),
            PathBuf::from("fragments/a/intro.md")
        );
        assert_eq!(
            shell_index_path(&dir, "design"),
            PathBuf::from("fragments/a/design/index.md")
        );
        assert_eq!(
            h3_leaf_path(&dir, "design", "flow"),
            PathBuf::from("fragments/a/design/flow.md")
        );
    }

    #[test]
    fn resolve_conflict_appends_suffix() {
        let mut used = HashSet::new();
        let first = resolve_conflict(PathBuf::from("fragments/a/index.md"), &mut used);
        let second = resolve_conflict(PathBuf::from("fragments/a/index.md"), &mut used);
        assert_eq!(first, PathBuf::from("fragments/a/index.md"));
        assert_eq!(second, PathBuf::from("fragments/a/index-1.md"));
    }
}
