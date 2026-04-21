pub mod fragment;
pub mod headings;
pub mod index;
pub mod links;
pub mod paths;
pub mod site_index;
pub mod tags;
pub mod unresolved;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::link::{LinkGraph, Resolver, UnresolvedLink};
use crate::model::{Node, iter_pages};

/// 生成物一式を出力ルートへ書き出す。毎回フル再生成（FR-03）。
pub struct WikiOutput<'a> {
    pub project_title: &'a str,
    pub nodes: &'a [Node],
    pub unresolved: &'a [UnresolvedLink],
    pub graph: &'a LinkGraph,
}

pub fn write_wiki(output_root: &Path, out: &WikiOutput<'_>) -> Result<()> {
    clean_output(output_root)?;
    fs::create_dir_all(output_root)
        .with_context(|| format!("failed to create {}", output_root.display()))?;

    let mut titles: std::collections::BTreeMap<std::path::PathBuf, String> =
        std::collections::BTreeMap::new();
    for n in out.nodes {
        for page in iter_pages(n) {
            titles.insert(page.output_path, page.title);
        }
    }

    let resolver = Resolver::build(out.nodes);

    for n in out.nodes {
        for page in fragment::render_pages(n, &titles, &resolver) {
            write_page(output_root, &page.output_path, &page.body)?;
        }
    }

    let tag_index = tags::build_tag_index(out.nodes);
    write_page(
        output_root,
        Path::new("tags/index.md"),
        &tags::render_tag_index_page(&tag_index),
    )?;
    for (tag, paths) in &tag_index.entries {
        let path = tags::tag_page_path(tag);
        let body = tags::render_tag_page(tag, paths, out.nodes);
        write_page(output_root, &path, &body)?;
    }

    write_page(
        output_root,
        Path::new("headings/index.md"),
        &headings::render_headings_index(out.nodes),
    )?;

    write_page(
        output_root,
        Path::new("links/index.md"),
        &links::render_links_index(out.nodes, out.graph),
    )?;

    write_page(
        output_root,
        Path::new("_unresolved.md"),
        &unresolved::render_unresolved(out.unresolved),
    )?;

    for page in site_index::render_site_indexes(out.nodes) {
        write_page(output_root, &page.output_path, &page.body)?;
    }

    let idx = index::render_index(out.project_title, out.nodes, out.unresolved, &tag_index);
    write_page(output_root, Path::new("index.md"), &idx)?;

    Ok(())
}

/// 出力先をクリーンアップする（FR-03）。
/// 既存が本ツール由来と推定できない場合はエラー終了する（§12 エラー処理）。
fn clean_output(output_root: &Path) -> Result<()> {
    if !output_root.exists() {
        return Ok(());
    }
    if !output_root.is_dir() {
        anyhow::bail!(
            "output path exists but is not a directory: {}",
            output_root.display()
        );
    }

    let is_empty = fs::read_dir(output_root)
        .map(|mut it| it.next().is_none())
        .unwrap_or(false);
    if is_empty {
        return Ok(());
    }

    let looks_like_ours =
        output_root.join("index.md").exists() || output_root.join("fragments").exists();
    if !looks_like_ours {
        anyhow::bail!(
            "refusing to clean {}: does not look like a md-wiki output directory \
             (no index.md or fragments/). Remove it manually or choose a different --out.",
            output_root.display()
        );
    }

    fs::remove_dir_all(output_root)
        .with_context(|| format!("failed to clear {}", output_root.display()))?;
    Ok(())
}

/// 1 ページ分の書き出しを集約するヘルパー。
fn write_page(root: &Path, rel: &Path, body: &str) -> Result<()> {
    let abs = root.join(rel);
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&abs, body).with_context(|| format!("failed to write {}", abs.display()))?;
    Ok(())
}
