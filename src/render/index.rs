use std::fmt::Write;

use super::tags::{TagIndex, tag_page_path};
use crate::extract::{EntryPoint, TechStack, TestLayout};
use crate::link::UnresolvedLink;
use crate::model::{Node, NodeKind};

/// `index.md` の本文を生成する（FR-08）。
pub fn render_index(
    project_title: &str,
    nodes: &[Node],
    tech_stack: &TechStack,
    entry_points: &[EntryPoint],
    test_layout: &TestLayout,
    unresolved: &[UnresolvedLink],
    tag_index: &TagIndex,
) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# {project_title}");
    s.push('\n');

    let _ = writeln!(&mut s, "## Tech stack");
    s.push('\n');
    if tech_stack.languages.is_empty() {
        let _ = writeln!(&mut s, "_(none detected)_");
    } else {
        let langs: Vec<&str> = tech_stack.languages.iter().map(String::as_str).collect();
        let _ = writeln!(&mut s, "{}", langs.join(" / "));
    }
    let _ = writeln!(
        &mut s,
        "\n詳細: [overview/tech-stack.md](overview/tech-stack.md)"
    );
    s.push('\n');

    let _ = writeln!(&mut s, "## Overview");
    s.push('\n');
    let _ = writeln!(&mut s, "- [Tech stack](overview/tech-stack.md)");
    let _ = writeln!(
        &mut s,
        "- [Entry points](overview/entry-points.md) ({} detected)",
        entry_points.len()
    );
    let _ = writeln!(
        &mut s,
        "- [Tests](overview/tests.md) ({} files)",
        test_layout.test_files.len()
    );
    let _ = writeln!(&mut s, "- [Development](development/index.md)");
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

    let note_nodes: Vec<&Node> = nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::NoteDerived))
        .collect();
    if !note_nodes.is_empty() {
        let _ = writeln!(&mut s, "## Notes");
        s.push('\n');
        for n in note_nodes {
            let _ = writeln!(&mut s, "- [{}]({})", n.title, n.output_path.display());
        }
        s.push('\n');
    }

    if !tag_index.entries.is_empty() {
        let _ = writeln!(&mut s, "## Tags");
        s.push('\n');
        for tag in tag_index.entries.keys() {
            let path = tag_page_path(tag);
            let _ = writeln!(
                &mut s,
                "- [`{tag}`]({})（{} 件）",
                path.display(),
                tag_index.entries[tag].len()
            );
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
