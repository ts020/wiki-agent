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

/// ノートの出力パスを算出する。
/// - `README.md` → `notes/README.md`
/// - `docs/foo.md` → `notes/docs/foo.md`
pub fn note_path(source_file: &Path) -> PathBuf {
    let sanitized = sanitize_path(source_file);
    PathBuf::from("notes").join(sanitized)
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
    fn note_path_for_root_file() {
        assert_eq!(
            note_path(Path::new("README.md")),
            PathBuf::from("notes/README.md")
        );
    }

    #[test]
    fn note_path_preserves_nested_structure() {
        assert_eq!(
            note_path(Path::new("docs/a/b.md")),
            PathBuf::from("notes/docs/a/b.md")
        );
    }

    #[test]
    fn note_path_sanitizes_forbidden_chars() {
        assert_eq!(
            note_path(Path::new("docs/a:b.md")),
            PathBuf::from("notes/docs/a_b.md")
        );
    }

    #[test]
    fn resolve_conflict_appends_suffix() {
        let mut used = HashSet::new();
        let first = resolve_conflict(PathBuf::from("notes/a.md"), &mut used);
        let second = resolve_conflict(PathBuf::from("notes/a.md"), &mut used);
        let third = resolve_conflict(PathBuf::from("notes/a.md"), &mut used);
        assert_eq!(first, PathBuf::from("notes/a.md"));
        assert_eq!(second, PathBuf::from("notes/a-1.md"));
        assert_eq!(third, PathBuf::from("notes/a-2.md"));
    }
}
