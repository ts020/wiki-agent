use std::fmt::Write;
use std::path::Path;

use crate::extract::{LocatedSymbol, SymbolKind, Visibility};
use crate::model::{Node, NodeKind};

pub fn render_node(
    node: &Node,
    titles: &std::collections::BTreeMap<std::path::PathBuf, String>,
) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# {}", node.title);
    s.push('\n');

    match node.kind {
        NodeKind::CodeDerived => render_code_node(&mut s, node),
        NodeKind::NoteDerived => render_note_node(&mut s, node),
    }

    render_relations(&mut s, node, titles);
    s
}

fn render_relations(
    s: &mut String,
    node: &Node,
    titles: &std::collections::BTreeMap<std::path::PathBuf, String>,
) {
    if !node.related.is_empty() {
        let _ = writeln!(s, "## Related");
        s.push('\n');
        for p in &node.related {
            render_linked_item(s, &node.output_path, p, titles);
        }
        s.push('\n');
    }
    if !node.read_next.is_empty() {
        let _ = writeln!(s, "## Read next");
        s.push('\n');
        for p in &node.read_next {
            render_linked_item(s, &node.output_path, p, titles);
        }
        s.push('\n');
    }
    if !node.backlinks.is_empty() {
        let _ = writeln!(s, "## Backlinks");
        s.push('\n');
        for p in &node.backlinks {
            render_linked_item(s, &node.output_path, p, titles);
        }
        s.push('\n');
    }
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

fn render_note_node(s: &mut String, node: &Node) {
    let Some(data) = &node.note else {
        return;
    };
    let _ = writeln!(s, "## Summary");
    s.push('\n');
    let summary = data
        .frontmatter
        .summary
        .clone()
        .or_else(|| data.first_paragraph.clone())
        .unwrap_or_else(|| "_(no summary)_".to_string());
    let _ = writeln!(s, "{summary}");
    s.push('\n');

    let _ = writeln!(s, "## Key files");
    s.push('\n');
    let _ = writeln!(s, "- `{}`", data.source_file.display());
    s.push('\n');

    let _ = writeln!(s, "## Structure");
    s.push('\n');
    if data.headings.is_empty() {
        let _ = writeln!(s, "_(no headings)_");
    } else {
        for h in &data.headings {
            let indent = "  ".repeat(h.level.saturating_sub(1) as usize);
            let _ = writeln!(s, "{indent}- {}", h.text);
        }
    }
    s.push('\n');

    if !data.frontmatter.tags.is_empty() {
        let _ = writeln!(s, "## Tags");
        s.push('\n');
        for t in &data.frontmatter.tags {
            let _ = writeln!(s, "- {t}");
        }
        s.push('\n');
    }

    if let Some(content_path) = &node.content_path {
        let link = super::paths::relative_link(&node.output_path, content_path);
        let _ = writeln!(s, "## Original");
        s.push('\n');
        let _ = writeln!(s, "- [原本を読む]({link})");
        s.push('\n');
    }
}

/// 原本コピー（`imported/<rel>`）用の本文を生成する。wikilink は
/// `content_resolver` で別ノートの `imported/` パスに解決される。
/// 本文は原本そのまま（見出しは重複させない）。末尾に索引ページへの
/// リンクを付与する。
pub fn render_imported(node: &Node, content_resolver: &crate::link::Resolver) -> Option<String> {
    let data = node.note.as_ref()?;
    let content_path = node.content_path.as_ref()?;
    let (body, _unresolved, _edges) =
        crate::link::wikilink::resolve_in(&data.body, content_path, content_resolver);

    let mut s = body;
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s.push_str("\n---\n\n");
    let back = super::paths::relative_link(content_path, &node.output_path);
    let _ = writeln!(&mut s, "_索引: [{}]({})_", node.title, back);
    Some(s)
}

/// 100件超過時に生成する `_symbols.md` の本文を生成する。
pub fn render_symbols_overflow(node: &Node) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Symbols — {}", node.title);
    s.push('\n');
    let _ = writeln!(&mut s, "ノード {} から退避した全シンボル。", node.title);
    s.push('\n');
    render_symbol_sections(&mut s, &node.symbols);
    s
}

fn render_code_node(s: &mut String, node: &Node) {
    let _ = writeln!(s, "## Summary");
    s.push('\n');
    let _ = writeln!(s, "- {}", summary_line(node));
    s.push('\n');

    let _ = writeln!(s, "## Key files");
    s.push('\n');
    if node.key_files.is_empty() {
        let _ = writeln!(s, "_(none)_");
    } else {
        for f in &node.key_files {
            let _ = writeln!(s, "- `{}`", f.display());
        }
    }
    s.push('\n');

    let _ = writeln!(s, "## Structure");
    s.push('\n');
    if let Some(op) = &node.symbols_overflow_path {
        let link = relative_link_from(&node.output_path, op);
        let _ = writeln!(
            s,
            "このディレクトリは {} 件のシンボルを含むため、全件は [`_symbols.md`]({}) を参照。",
            node.symbols.len(),
            link
        );
    } else if node.symbols.is_empty() {
        let _ = writeln!(s, "_(no top-level symbols detected)_");
    } else {
        render_symbol_sections(s, &node.symbols);
    }
    s.push('\n');
}

fn summary_line(node: &Node) -> String {
    if node.symbols.is_empty() {
        return format!(
            "{} file(s), no top-level symbols detected",
            node.key_files.len()
        );
    }
    let pub_count = node
        .symbols
        .iter()
        .filter(|s| s.symbol.visibility == Visibility::Public)
        .count();
    let priv_count = node.symbols.len() - pub_count;
    let mut types = 0;
    let mut fns = 0;
    let mut consts = 0;
    for s in &node.symbols {
        match s.symbol.kind {
            SymbolKind::Type => types += 1,
            SymbolKind::Function => fns += 1,
            SymbolKind::Constant => consts += 1,
        }
    }
    format!(
        "{} file(s); {} symbol(s) — {} public / {} private — types: {}, fns: {}, consts: {}",
        node.key_files.len(),
        node.symbols.len(),
        pub_count,
        priv_count,
        types,
        fns,
        consts,
    )
}

fn render_symbol_sections(s: &mut String, symbols: &[LocatedSymbol]) {
    let pubs: Vec<&LocatedSymbol> = symbols
        .iter()
        .filter(|x| x.symbol.visibility == Visibility::Public)
        .collect();
    let privs: Vec<&LocatedSymbol> = symbols
        .iter()
        .filter(|x| x.symbol.visibility == Visibility::Private)
        .collect();

    let _ = writeln!(s, "### Public");
    s.push('\n');
    if pubs.is_empty() {
        let _ = writeln!(s, "_(none)_");
    } else {
        for sym in pubs {
            let _ = writeln!(
                s,
                "- `{}` ({}) — `{}`",
                sym.symbol.name,
                sym.symbol.kind.label(),
                sym.source.display()
            );
        }
    }
    s.push('\n');

    let _ = writeln!(s, "### Private");
    s.push('\n');
    if privs.is_empty() {
        let _ = writeln!(s, "_(none)_");
    } else {
        for sym in privs {
            let _ = writeln!(
                s,
                "- `{}` ({}) — `{}`",
                sym.symbol.name,
                sym.symbol.kind.label(),
                sym.source.display()
            );
        }
    }
    s.push('\n');
}

/// `from` からみた `to` への相対リンク文字列を生成する（どちらも出力ルート相対）。
fn relative_link_from(from: &Path, to: &Path) -> String {
    let from_parent = from.parent().unwrap_or(Path::new(""));
    PathDiff::new(from_parent, to).to_string()
}

struct PathDiff<'a> {
    from: &'a Path,
    to: &'a Path,
}

impl<'a> PathDiff<'a> {
    fn new(from: &'a Path, to: &'a Path) -> Self {
        Self { from, to }
    }
}

impl<'a> std::fmt::Display for PathDiff<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::path::Component;
        let from_comps: Vec<_> = self
            .from
            .components()
            .filter(|c| matches!(c, Component::Normal(_)))
            .collect();
        let to_comps: Vec<_> = self
            .to
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
            write!(f, ".")
        } else {
            write!(f, "{}", parts.join("/"))
        }
    }
}
