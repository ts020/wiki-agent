use std::fmt::Write;
use std::path::PathBuf;

use super::paths::relative_link;
use super::tags::TagIndex;
use crate::link::UnresolvedLink;
use crate::model::Node;

/// ルート `index.md` の本文を生成する（FR-12）。
pub fn render_index(
    project_title: &str,
    nodes: &[Node],
    unresolved: &[UnresolvedLink],
    tag_index: &TagIndex,
) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# {project_title}");
    s.push('\n');

    let _ = writeln!(&mut s, "## Summary");
    s.push('\n');
    let _ = writeln!(&mut s, "- Notes: {}", nodes.len());
    let _ = writeln!(&mut s, "- Tags: {}", tag_index.entries.len());
    let _ = writeln!(&mut s, "- Unresolved links: {}", unresolved.len());
    s.push('\n');

    let _ = writeln!(&mut s, "## Sections");
    s.push('\n');
    let _ = writeln!(&mut s, "- [Tags](tags/index.md)");
    let _ = writeln!(&mut s, "- [Headings](headings/index.md)");
    let _ = writeln!(&mut s, "- [Links](links/index.md)");
    s.push('\n');

    if !nodes.is_empty() {
        let _ = writeln!(&mut s, "## Notes");
        s.push('\n');
        let from = PathBuf::from("index.md");
        for n in nodes {
            let link = relative_link(&from, &n.output_path);
            let _ = writeln!(&mut s, "- [{}]({})", n.title, link);
        }
        s.push('\n');
    }

    if !unresolved.is_empty() {
        let _ = writeln!(
            &mut s,
            "## Unresolved links\n\n未解決の wikilink が {} 件あります。詳細は [_unresolved.md](_unresolved.md)。",
            unresolved.len()
        );
        s.push('\n');
    }
    s
}
