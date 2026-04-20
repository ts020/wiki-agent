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

/// コード由来ノードの出力パスを算出する。
///
/// - ルート (`""` 相当) → `directories/_root.md`
/// - `src` → `directories/src.md`
/// - `src/scan` → `directories/src/scan.md`
pub fn code_node_path(source_dir: &Path) -> PathBuf {
    let sanitized = sanitize_path(source_dir);
    if sanitized.as_os_str().is_empty() {
        return PathBuf::from("directories").join("_root.md");
    }
    let mut out = PathBuf::from("directories").join(&sanitized);
    out.set_extension("md");
    out
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
    fn code_path_for_root_dir() {
        assert_eq!(
            code_node_path(Path::new("")),
            PathBuf::from("directories/_root.md")
        );
    }

    #[test]
    fn code_path_for_nested_dir() {
        assert_eq!(
            code_node_path(Path::new("src/scan")),
            PathBuf::from("directories/src/scan.md")
        );
    }

    #[test]
    fn code_path_sanitizes_components() {
        assert_eq!(
            code_node_path(Path::new("a:b/c")),
            PathBuf::from("directories/a_b/c.md")
        );
    }

    #[test]
    fn resolve_conflict_appends_suffix() {
        let mut used = HashSet::new();
        let first = resolve_conflict(PathBuf::from("directories/a.md"), &mut used);
        let second = resolve_conflict(PathBuf::from("directories/a.md"), &mut used);
        let third = resolve_conflict(PathBuf::from("directories/a.md"), &mut used);
        assert_eq!(first, PathBuf::from("directories/a.md"));
        assert_eq!(second, PathBuf::from("directories/a-1.md"));
        assert_eq!(third, PathBuf::from("directories/a-2.md"));
    }
}
