use std::fmt::Write;

use crate::model::{Node, NodeKind};

/// ノード 1 件分の Markdown を生成する。
pub fn render_node(node: &Node) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# {}", node.title);
    writeln_blank(&mut s);

    match node.kind {
        NodeKind::CodeDerived => render_code_node(&mut s, node),
    }

    s
}

fn render_code_node(s: &mut String, node: &Node) {
    let _ = writeln!(s, "## Key files");
    writeln_blank(s);
    if node.key_files.is_empty() {
        let _ = writeln!(s, "_(none)_");
    } else {
        for f in &node.key_files {
            let _ = writeln!(s, "- `{}`", f.display());
        }
    }
    writeln_blank(s);
}

fn writeln_blank(s: &mut String) {
    s.push('\n');
}
