use std::fmt::Write;

use crate::link::UnresolvedLink;

/// `_unresolved.md` の本文を生成する。未解決リンクが無い場合でもファイルは作成し、
/// 旨を明記する（増築後の差分確認に使うため）。
pub fn render_unresolved(list: &[UnresolvedLink]) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Unresolved wikilinks");
    s.push('\n');
    if list.is_empty() {
        let _ = writeln!(&mut s, "_(すべての wikilink が解決されました)_");
        return s;
    }
    let _ = writeln!(&mut s, "| 参照元 | ターゲット | 見出し | 表示名 |");
    let _ = writeln!(&mut s, "|--------|------------|--------|--------|");
    for u in list {
        let _ = writeln!(
            &mut s,
            "| `{}` | `{}` | {} | {} |",
            u.source.display(),
            u.target,
            u.heading.as_deref().unwrap_or("-"),
            u.alias.as_deref().unwrap_or("-"),
        );
    }
    s
}
