use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;

use super::paths::relative_link;
use crate::link::LinkGraph;
use crate::model::Node;

/// `links/index.md` の本文を生成する（FR-09）。
/// 各ノートの「このノートが参照しているノート」を一覧化する。
/// 未解決リンクは `_unresolved.md` に集約するためここには含めない。
pub fn render_links_index(nodes: &[Node], graph: &LinkGraph) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Links");
    s.push('\n');

    let from = PathBuf::from("links/index.md");
    let titles: BTreeMap<PathBuf, String> = nodes
        .iter()
        .map(|n| (n.output_path.clone(), n.title.clone()))
        .collect();

    let mut any = false;
    for n in nodes {
        let forward = graph.forward_of(&n.output_path);
        if forward.is_empty() {
            continue;
        }
        any = true;
        let self_link = relative_link(&from, &n.output_path);
        let _ = writeln!(&mut s, "## [{}]({})", n.title, self_link);
        s.push('\n');
        for target in forward {
            let title = titles
                .get(&target)
                .cloned()
                .unwrap_or_else(|| target.display().to_string());
            let link = relative_link(&from, &target);
            let _ = writeln!(&mut s, "- [{title}]({link})");
        }
        s.push('\n');
    }

    if !any {
        let _ = writeln!(&mut s, "_(no links between notes)_");
    }
    s
}
