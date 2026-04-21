//! 入口ページ・殻ページ・断片ページのレンダリング（FR-05, FR-05a, FR-10, FR-11）。

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use super::paths::{fragment_leaf_path, h3_leaf_path, relative_link, shell_index_path};
use crate::fragment::{Fragment, FragmentTree};
use crate::link::{Resolver, wikilink};
use crate::model::Node;

/// 1 ページ分のレンダリング結果。
pub struct PageRender {
    pub output_path: PathBuf,
    pub body: String,
}

/// 1 ノートに属するすべてのページ（入口・殻・h2 断片・h3 子断片）を書き出す。
pub fn render_pages(
    node: &Node,
    titles: &BTreeMap<PathBuf, String>,
    resolver: &Resolver,
) -> Vec<PageRender> {
    let mut pages = Vec::new();
    pages.push(PageRender {
        output_path: node.output_path.clone(),
        body: render_entry(node, titles, resolver),
    });
    for (idx, frag) in node.fragments.fragments.iter().enumerate() {
        match frag {
            Fragment::H2 { slug, .. } => {
                let path = fragment_leaf_path(&node.entry_dir, slug);
                let body = render_h2_leaf(node, idx, &path, titles, resolver);
                pages.push(PageRender {
                    output_path: path,
                    body,
                });
            }
            Fragment::Shell { slug, children, .. } => {
                let shell_path = shell_index_path(&node.entry_dir, slug);
                let shell_body = render_shell(node, idx, &shell_path, titles, resolver);
                pages.push(PageRender {
                    output_path: shell_path,
                    body: shell_body,
                });
                for (cidx, child) in children.iter().enumerate() {
                    let path = h3_leaf_path(&node.entry_dir, slug, &child.slug);
                    let body = render_h3_leaf(node, idx, cidx, &path, titles, resolver);
                    pages.push(PageRender {
                        output_path: path,
                        body,
                    });
                }
            }
        }
    }
    pages
}

fn render_entry(node: &Node, titles: &BTreeMap<PathBuf, String>, resolver: &Resolver) -> String {
    let from = &node.output_path;
    let (preface, _, _) = wikilink::resolve_in(&node.fragments.preface, from, from, resolver);

    let fragments_section = build_fragments_section(node, from);
    let backlinks_section = build_backlinks_section(node, from, titles);
    let related_section = build_related_section(node, from, titles);

    assemble(
        None,
        preface,
        &[fragments_section, backlinks_section, related_section],
    )
}

fn render_h2_leaf(
    node: &Node,
    idx: usize,
    page_path: &Path,
    titles: &BTreeMap<PathBuf, String>,
    resolver: &Resolver,
) -> String {
    let body_raw = match &node.fragments.fragments[idx] {
        Fragment::H2 { body, .. } => body.as_str(),
        _ => unreachable!("render_h2_leaf called on non-H2 fragment"),
    };
    let (body, _, _) = wikilink::resolve_in(body_raw, page_path, &node.output_path, resolver);
    let head = build_h2_nav(node, idx, page_path);
    let backlinks = build_backlinks_section(node, page_path, titles);
    assemble(Some(head), body, &[backlinks])
}

fn render_shell(
    node: &Node,
    idx: usize,
    page_path: &Path,
    titles: &BTreeMap<PathBuf, String>,
    resolver: &Resolver,
) -> String {
    let preface_raw = match &node.fragments.fragments[idx] {
        Fragment::Shell { preface, .. } => preface.as_str(),
        _ => unreachable!("render_shell called on non-Shell fragment"),
    };
    let (preface, _, _) = wikilink::resolve_in(preface_raw, page_path, &node.output_path, resolver);
    let head = build_shell_nav(node, page_path);
    let children_section = build_shell_children_section(node, idx, page_path);
    let backlinks = build_backlinks_section(node, page_path, titles);
    assemble(Some(head), preface, &[children_section, backlinks])
}

fn build_shell_children_section(node: &Node, idx: usize, page_path: &Path) -> String {
    let (children, h2_slug) = match &node.fragments.fragments[idx] {
        Fragment::Shell { children, slug, .. } => (children, slug.as_str()),
        _ => return String::new(),
    };
    if children.is_empty() {
        return String::new();
    }
    let mut s = String::from("## Fragments\n\n");
    for child in children {
        let target = h3_leaf_path(&node.entry_dir, h2_slug, &child.slug);
        let link = relative_link(page_path, &target);
        let _ = writeln!(&mut s, "- [{}]({link})", child.heading);
    }
    s
}

fn render_h3_leaf(
    node: &Node,
    h2_idx: usize,
    h3_idx: usize,
    page_path: &Path,
    titles: &BTreeMap<PathBuf, String>,
    resolver: &Resolver,
) -> String {
    let body_raw = match &node.fragments.fragments[h2_idx] {
        Fragment::Shell { children, .. } => children[h3_idx].body.as_str(),
        _ => unreachable!("render_h3_leaf called on non-Shell fragment"),
    };
    let (body, _, _) = wikilink::resolve_in(body_raw, page_path, &node.output_path, resolver);
    let head = build_h3_nav(node, h2_idx, h3_idx, page_path);
    let backlinks = build_backlinks_section(node, page_path, titles);
    assemble(Some(head), body, &[backlinks])
}

fn assemble(head: Option<String>, main: String, autos: &[String]) -> String {
    let autos: Vec<&String> = autos.iter().filter(|s| !s.is_empty()).collect();
    let mut s = String::new();
    if let Some(h) = head {
        s.push_str(&h);
        ensure_trailing_newline(&mut s);
        s.push_str("\n---\n\n");
    }
    s.push_str(&main);
    if !autos.is_empty() {
        ensure_trailing_newline(&mut s);
        s.push_str("\n---\n\n");
        for (i, part) in autos.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            s.push_str(part);
        }
    }
    s
}

fn ensure_trailing_newline(s: &mut String) {
    if !s.ends_with('\n') {
        s.push('\n');
    }
}

fn build_fragments_section(node: &Node, from: &Path) -> String {
    let tree = &node.fragments;
    if tree.non_fragmented || tree.fragments.is_empty() {
        return String::new();
    }
    let mut s = String::from("## Fragments\n\n");
    for frag in &tree.fragments {
        let target = match frag {
            Fragment::H2 { slug, .. } => fragment_leaf_path(&node.entry_dir, slug),
            Fragment::Shell { slug, .. } => shell_index_path(&node.entry_dir, slug),
        };
        let link = relative_link(from, &target);
        let _ = writeln!(&mut s, "- [{}]({link})", frag.heading());
    }
    s
}

fn build_backlinks_section(node: &Node, from: &Path, titles: &BTreeMap<PathBuf, String>) -> String {
    let links = match node.backlinks.get(from) {
        Some(v) if !v.is_empty() => v,
        _ => return String::new(),
    };
    let mut s = String::from("## Backlinks\n\n");
    for p in links {
        append_link_item(&mut s, from, p, titles);
    }
    s
}

fn build_related_section(node: &Node, from: &Path, titles: &BTreeMap<PathBuf, String>) -> String {
    if node.related.is_empty() {
        return String::new();
    }
    let mut s = String::from("## Related\n\n");
    for p in &node.related {
        append_link_item(&mut s, from, p, titles);
    }
    s
}

fn append_link_item(s: &mut String, from: &Path, to: &Path, titles: &BTreeMap<PathBuf, String>) {
    let title = titles
        .get(to)
        .cloned()
        .unwrap_or_else(|| to.display().to_string());
    let link = relative_link(from, to);
    let _ = writeln!(s, "- [{title}]({link})");
}

fn build_h2_nav(node: &Node, idx: usize, page_path: &Path) -> String {
    let mut parts = vec![format!(
        "Parent: [{}]({})",
        node.title,
        relative_link(page_path, &node.output_path)
    )];
    if idx > 0 {
        let prev = &node.fragments.fragments[idx - 1];
        parts.push(format!(
            "Prev: [{}]({})",
            prev.heading(),
            relative_link(page_path, &fragment_target(node, prev))
        ));
    }
    if let Some(next) = node.fragments.fragments.get(idx + 1) {
        parts.push(format!(
            "Next: [{}]({})",
            next.heading(),
            relative_link(page_path, &fragment_target(node, next))
        ));
    }
    format!("> {}", parts.join(" · "))
}

fn build_shell_nav(node: &Node, page_path: &Path) -> String {
    format!(
        "> Parent: [{}]({})",
        node.title,
        relative_link(page_path, &node.output_path)
    )
}

fn build_h3_nav(node: &Node, h2_idx: usize, h3_idx: usize, page_path: &Path) -> String {
    let (h2_heading, h2_slug, children) = match &node.fragments.fragments[h2_idx] {
        Fragment::Shell {
            heading,
            slug,
            children,
            ..
        } => (heading.clone(), slug.clone(), children),
        _ => unreachable!(),
    };
    let shell_path = shell_index_path(&node.entry_dir, &h2_slug);
    let mut parts = vec![format!(
        "Parent: [{}]({})",
        h2_heading,
        relative_link(page_path, &shell_path)
    )];
    if h3_idx > 0 {
        let prev = &children[h3_idx - 1];
        parts.push(format!(
            "Prev: [{}]({})",
            prev.heading,
            relative_link(
                page_path,
                &h3_leaf_path(&node.entry_dir, &h2_slug, &prev.slug)
            )
        ));
    }
    if let Some(next) = children.get(h3_idx + 1) {
        parts.push(format!(
            "Next: [{}]({})",
            next.heading,
            relative_link(
                page_path,
                &h3_leaf_path(&node.entry_dir, &h2_slug, &next.slug)
            )
        ));
    }
    format!("> {}", parts.join(" · "))
}

fn fragment_target(node: &Node, frag: &Fragment) -> PathBuf {
    match frag {
        Fragment::H2 { slug, .. } => fragment_leaf_path(&node.entry_dir, slug),
        Fragment::Shell { slug, .. } => shell_index_path(&node.entry_dir, slug),
    }
}

#[allow(dead_code)]
fn _assert_types(_: &FragmentTree) {}
