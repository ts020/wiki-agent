use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;

use crate::link::UnresolvedLink;

/// `_unresolved.md` の本文を生成する（FR-13）。
///
/// 未解決リンクを**参照元ページ単位**でグループ化し、各グループを `## <path>`
/// 見出しとリスト形式で出力する。`resolve_all` の段階で
/// `(source, target)` 昇順ソートされている前提（グループ内順は本文出現順にほぼ等しい）。
pub fn render_unresolved(list: &[UnresolvedLink]) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Unresolved wikilinks");
    s.push('\n');
    if list.is_empty() {
        let _ = writeln!(&mut s, "_(すべての wikilink が解決されました)_");
        return s;
    }

    let mut by_source: BTreeMap<PathBuf, Vec<&UnresolvedLink>> = BTreeMap::new();
    for u in list {
        by_source.entry(u.source.clone()).or_default().push(u);
    }
    for (source, items) in &by_source {
        let _ = writeln!(&mut s, "## `{}`", source.display());
        s.push('\n');
        for u in items {
            let mut notation = format!("[[{}", u.target);
            if let Some(h) = &u.heading {
                notation.push('#');
                notation.push_str(h);
            }
            if let Some(a) = &u.alias {
                notation.push('|');
                notation.push_str(a);
            }
            notation.push_str("]]");
            let reason = if u.heading.is_some() {
                " — 対象ノート内に見出しが見つからない"
            } else {
                " — 対象ノートが見つからない"
            };
            let _ = writeln!(&mut s, "- `{notation}`{reason}");
        }
        s.push('\n');
    }
    s
}
