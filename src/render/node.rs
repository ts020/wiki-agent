use std::fmt::Write;

use crate::model::Node;

/// ノートページの本文を生成する。原本本文に wikilink を変換した結果を置き、
/// 末尾に `## Backlinks` と `## Related` を水平線で区切って付与する（FR-05）。
pub fn render_node(
    node: &Node,
    titles: &std::collections::BTreeMap<std::path::PathBuf, String>,
    resolver: &crate::link::Resolver,
) -> String {
    let (body, _unresolved, _edges) =
        crate::link::wikilink::resolve_in(&node.note.body, &node.output_path, resolver);

    let mut s = body;

    let has_backlinks = !node.backlinks.is_empty();
    let has_related = !node.related.is_empty();
    if !has_backlinks && !has_related {
        return s;
    }

    if !s.ends_with('\n') {
        s.push('\n');
    }
    s.push_str("\n---\n\n");

    if has_backlinks {
        let _ = writeln!(&mut s, "## Backlinks");
        s.push('\n');
        for p in &node.backlinks {
            render_linked_item(&mut s, &node.output_path, p, titles);
        }
        s.push('\n');
    }
    if has_related {
        let _ = writeln!(&mut s, "## Related");
        s.push('\n');
        for p in &node.related {
            render_linked_item(&mut s, &node.output_path, p, titles);
        }
        s.push('\n');
    }

    s
}

fn render_linked_item(
    s: &mut String,
    from: &std::path::Path,
    to: &std::path::Path,
    titles: &std::collections::BTreeMap<std::path::PathBuf, String>,
) {
    let title = titles
        .get(to)
        .cloned()
        .unwrap_or_else(|| to.display().to_string());
    let link = super::paths::relative_link(from, to);
    let _ = writeln!(s, "- [{title}]({link})");
}
