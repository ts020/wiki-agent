use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;

use super::paths::relative_link;
use super::text::link_label;
use crate::link::LinkGraph;
use crate::model::{Node, iter_pages};

/// `links/index.md` の本文を生成する（FR-09）。
/// 断片解像度で各ページの参照先を列挙する。未解決リンクは含めない。
pub fn render_links_index(nodes: &[Node], graph: &LinkGraph) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Links");
    s.push('\n');

    let from = PathBuf::from("links/index.md");

    // ページタイトル表: 出力パス → タイトル。全ノートの全ページ分。
    let mut titles: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut ordered_pages: Vec<(PathBuf, String)> = Vec::new();
    for n in nodes {
        for page in iter_pages(n) {
            titles.insert(page.output_path.clone(), page.title.clone());
            ordered_pages.push((page.output_path, page.title));
        }
    }
    ordered_pages.sort_by(|a, b| a.0.cmp(&b.0));

    let mut any = false;
    for (page_path, title) in &ordered_pages {
        let forward = graph.forward_of(page_path);
        if forward.is_empty() {
            continue;
        }
        any = true;
        let self_link = relative_link(&from, page_path);
        let _ = writeln!(&mut s, "## [{}]({self_link})", link_label(title));
        s.push('\n');
        for target in forward {
            let target_title = titles
                .get(&target)
                .cloned()
                .unwrap_or_else(|| target.display().to_string());
            let link = relative_link(&from, &target);
            let _ = writeln!(&mut s, "- [{}]({link})", link_label(&target_title));
        }
        s.push('\n');
    }

    if !any {
        let _ = writeln!(&mut s, "_(no links between pages)_");
    }
    s
}
