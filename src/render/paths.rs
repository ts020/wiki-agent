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
/// - ルート (`""` 相当) → `code-nodes/_root.md`
/// - `src` → `code-nodes/src.md`
/// - `src/scan` → `code-nodes/src/scan.md`
pub fn code_node_path(source_dir: &Path) -> PathBuf {
    let sanitized = sanitize_path(source_dir);
    if sanitized.as_os_str().is_empty() {
        return PathBuf::from("code-nodes").join("_root.md");
    }
    let mut out = PathBuf::from("code-nodes").join(&sanitized);
    out.set_extension("md");
    out
}

/// ノート索引ページの出力パスを算出する。
/// - `README.md` → `note-index/README.md`
/// - `docs/foo.md` → `note-index/docs/foo.md`
pub fn note_index_path(source_file: &Path) -> PathBuf {
    let sanitized = sanitize_path(source_file);
    PathBuf::from("note-index").join(sanitized)
}

/// ノート原本コピーの出力パスを算出する。
/// - `README.md` → `imported/README.md`
/// - `docs/foo.md` → `imported/docs/foo.md`
pub fn imported_note_path(source_file: &Path) -> PathBuf {
    let sanitized = sanitize_path(source_file);
    PathBuf::from("imported").join(sanitized)
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
    fn code_path_for_root_dir() {
        assert_eq!(
            code_node_path(Path::new("")),
            PathBuf::from("code-nodes/_root.md")
        );
    }

    #[test]
    fn code_path_for_nested_dir() {
        assert_eq!(
            code_node_path(Path::new("src/scan")),
            PathBuf::from("code-nodes/src/scan.md")
        );
    }

    #[test]
    fn code_path_sanitizes_components() {
        assert_eq!(
            code_node_path(Path::new("a:b/c")),
            PathBuf::from("code-nodes/a_b/c.md")
        );
    }

    #[test]
    fn note_index_path_for_root_readme() {
        assert_eq!(
            note_index_path(Path::new("README.md")),
            PathBuf::from("note-index/README.md")
        );
    }

    #[test]
    fn note_index_path_preserves_nested_structure() {
        assert_eq!(
            note_index_path(Path::new("docs/a/b.md")),
            PathBuf::from("note-index/docs/a/b.md")
        );
    }

    #[test]
    fn imported_path_for_root_readme() {
        assert_eq!(
            imported_note_path(Path::new("README.md")),
            PathBuf::from("imported/README.md")
        );
    }

    #[test]
    fn imported_path_preserves_nested_structure() {
        assert_eq!(
            imported_note_path(Path::new("docs/a/b.md")),
            PathBuf::from("imported/docs/a/b.md")
        );
    }

    #[test]
    fn note_path_sanitizes_forbidden_chars() {
        assert_eq!(
            note_index_path(Path::new("docs/a:b.md")),
            PathBuf::from("note-index/docs/a_b.md")
        );
        assert_eq!(
            imported_note_path(Path::new("docs/a:b.md")),
            PathBuf::from("imported/docs/a_b.md")
        );
    }

    #[test]
    fn resolve_conflict_appends_suffix() {
        let mut used = HashSet::new();
        let first = resolve_conflict(PathBuf::from("code-nodes/a.md"), &mut used);
        let second = resolve_conflict(PathBuf::from("code-nodes/a.md"), &mut used);
        let third = resolve_conflict(PathBuf::from("code-nodes/a.md"), &mut used);
        assert_eq!(first, PathBuf::from("code-nodes/a.md"));
        assert_eq!(second, PathBuf::from("code-nodes/a-1.md"));
        assert_eq!(third, PathBuf::from("code-nodes/a-2.md"));
    }
}
