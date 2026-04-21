use std::ops::Range;
use std::path::{Component, Path};
use std::sync::OnceLock;

use regex::Regex;

use super::resolver::{Resolution, Resolver, UnresolvedLink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    pub embed: bool,
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
}

static RE: OnceLock<Regex> = OnceLock::new();

fn wikilink_re() -> &'static Regex {
    RE.get_or_init(|| Regex::new(r"(!)?\[\[([^\[\]\n]+)\]\]").unwrap())
}

/// 本文からすべての wikilink を抽出する。三重バッククォート/チルダの
/// フェンスドコードブロック内はスキップする。
pub fn find_all(body: &str) -> Vec<(Range<usize>, WikiLink)> {
    let mut out = Vec::new();
    let mut offset: usize = 0;
    let mut in_fence = false;
    let re = wikilink_re();
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            for m in re.captures_iter(line) {
                let full = m.get(0).unwrap();
                let embed = m.get(1).is_some();
                let content = m.get(2).unwrap().as_str();
                let link = parse_content(content, embed);
                out.push((offset + full.start()..offset + full.end(), link));
            }
        }
        offset += line.len();
    }
    out
}

fn parse_content(s: &str, embed: bool) -> WikiLink {
    let (before_alias, alias) = match s.split_once('|') {
        Some((a, b)) => (a, Some(b.trim().to_string())),
        None => (s, None),
    };
    let (target, heading) = match before_alias.split_once('#') {
        Some((a, b)) => (a.trim(), Some(b.trim().to_string())),
        None => (before_alias.trim(), None),
    };
    WikiLink {
        embed,
        target: target.to_string(),
        heading,
        alias,
    }
}

/// 本文中の wikilink を解決した新しい本文、未解決一覧、解決済みリンクの
/// ターゲット一覧を返す。`from` は現ページの出力相対パス（相対リンク生成に使用）、
/// `anchor` は参照元ノートの入口ページ（Resolver のディレクトリ近接判定に使用）。
pub fn resolve_in(
    body: &str,
    from: &Path,
    anchor: &Path,
    resolver: &Resolver,
) -> (String, Vec<UnresolvedLink>, Vec<std::path::PathBuf>) {
    let links = find_all(body);
    let mut out = body.to_string();
    let mut unresolved = Vec::new();
    let mut edges = Vec::new();

    for (range, link) in links.into_iter().rev() {
        let display = link.alias.clone().unwrap_or_else(|| match &link.heading {
            Some(h) => format!("{}#{}", link.target, h),
            None => link.target.clone(),
        });

        let resolution =
            resolver.resolve_with_heading(&link.target, link.heading.as_deref(), anchor);
        match resolution {
            Resolution::Entry(target_path) => {
                edges.push(target_path.clone());
                let link_text = render_relative(from, &target_path);
                out.replace_range(range, &format!("[{display}]({link_text})"));
            }
            Resolution::EntryAnchor(target_path, slug) => {
                edges.push(target_path.clone());
                let mut link_text = render_relative(from, &target_path);
                link_text.push('#');
                link_text.push_str(&slug);
                out.replace_range(range, &format!("[{display}]({link_text})"));
            }
            Resolution::Page(page_path) => {
                edges.push(page_path.clone());
                let link_text = render_relative(from, &page_path);
                out.replace_range(range, &format!("[{display}]({link_text})"));
            }
            Resolution::Missing => {
                let original = &body[range.clone()];
                let replacement = format!("{original} (未解決)");
                out.replace_range(range, &replacement);
                unresolved.push(UnresolvedLink {
                    source: from.to_path_buf(),
                    target: link.target,
                    heading: link.heading,
                    alias: link.alias,
                });
            }
        }
    }
    (out, unresolved, edges)
}

/// `from` (出力相対パス) からみた `to` への相対リンクを返す。
pub(crate) fn render_relative(from: &Path, to: &Path) -> String {
    let from_parent = from.parent().unwrap_or(Path::new(""));
    let from_comps: Vec<_> = from_parent
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .collect();
    let to_comps: Vec<_> = to
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
        ".".to_string()
    } else {
        parts.join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_wikilink() {
        let links = find_all("Hello [[Foo]] world");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].1.target, "Foo");
        assert!(links[0].1.heading.is_none());
        assert!(links[0].1.alias.is_none());
        assert!(!links[0].1.embed);
    }

    #[test]
    fn parses_alias_and_heading() {
        let links = find_all("[[Foo#Section|display]]");
        assert_eq!(links[0].1.target, "Foo");
        assert_eq!(links[0].1.heading.as_deref(), Some("Section"));
        assert_eq!(links[0].1.alias.as_deref(), Some("display"));
    }

    #[test]
    fn parses_embed() {
        let links = find_all("see ![[Pic]]");
        assert_eq!(links[0].1.target, "Pic");
        assert!(links[0].1.embed);
    }

    #[test]
    fn skips_wikilinks_in_fenced_code_blocks() {
        let body = "real [[Yes]]\n```\n[[No]]\n```\nafter [[Also]]";
        let links = find_all(body);
        let targets: Vec<_> = links.iter().map(|(_, l)| l.target.clone()).collect();
        assert_eq!(targets, vec!["Yes", "Also"]);
    }

    #[test]
    fn relative_path_computation() {
        assert_eq!(
            render_relative(Path::new("notes/a/b.md"), Path::new("notes/a/c.md")),
            "c.md"
        );
        assert_eq!(
            render_relative(
                Path::new("note-index/a/b.md"),
                Path::new("code-nodes/src.md")
            ),
            "../../code-nodes/src.md"
        );
    }
}
