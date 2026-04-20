use std::fmt::Write;

use crate::model::{Node, NodeKind};

/// `index.md` の内容を生成する。Phase 2 では最小限（プロジェクト名 + ディレクトリ一覧）。
pub fn render_index(project_title: &str, nodes: &[Node]) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# {project_title}");
    s.push('\n');
    let _ = writeln!(&mut s, "## Directories");
    s.push('\n');

    let code_nodes: Vec<&Node> = nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::CodeDerived))
        .collect();

    if code_nodes.is_empty() {
        let _ = writeln!(&mut s, "_(none)_");
    } else {
        for n in code_nodes {
            let _ = writeln!(&mut s, "- [{}]({})", n.title, n.output_path.display());
        }
    }
    s.push('\n');
    s
}
