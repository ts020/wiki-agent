pub mod fragment;
pub mod headings;
pub mod index;
pub mod links;
pub mod paths;
pub mod site_index;
pub mod tags;
pub mod text;
pub mod unresolved;

use std::path::Path;

use anyhow::Result;

use crate::agentic_output::finalize_agentic_plan;
use crate::link::{LinkGraph, Resolver, UnresolvedLink};
use crate::metadata_renderer::markdown_path;
use crate::model::{Node, iter_pages};
use crate::output_plan::{OutputPlan, insert_text, write_plan_to_clean_dir};

/// 128k コンテキスト前提でのページサイズ上限（§9, AC-23）。超過時は warn ログ。
const PAGE_CHAR_LIMIT: usize = 40_000;

/// 生成物一式を組み立てるための入力。
pub struct WikiOutput<'a> {
    pub project_title: &'a str,
    pub nodes: &'a [Node],
    pub unresolved: &'a [UnresolvedLink],
    pub graph: &'a LinkGraph,
}

pub fn build_core_wiki_plan(out: &WikiOutput<'_>) -> Result<OutputPlan> {
    let mut plan = OutputPlan::new();
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
            write_page(&mut plan, &page.output_path, &page.body);
        }
    }

    let tag_index = tags::build_tag_index(out.nodes);
    write_page(
        &mut plan,
        Path::new("tags/index.md"),
        &tags::render_tag_index_page(&tag_index),
    );
    for (tag, paths) in &tag_index.entries {
        let path = tags::tag_page_path(tag);
        let body = tags::render_tag_page(tag, paths, out.nodes);
        write_page(&mut plan, &path, &body);
    }

    write_page(
        &mut plan,
        Path::new("headings/index.md"),
        &headings::render_headings_index(out.nodes),
    );

    write_page(
        &mut plan,
        Path::new("links/index.md"),
        &links::render_links_index(out.nodes, out.graph),
    );

    write_page(
        &mut plan,
        Path::new("_unresolved.md"),
        &unresolved::render_unresolved(out.unresolved),
    );

    for page in site_index::render_site_indexes(out.nodes) {
        write_page(&mut plan, &page.output_path, &page.body);
    }

    let idx = index::render_index(out.project_title, out.nodes, out.unresolved, &tag_index);
    write_page(&mut plan, Path::new("index.md"), &idx);

    Ok(plan)
}

pub fn build_wiki_plan(out: &WikiOutput<'_>) -> Result<OutputPlan> {
    let mut plan = build_core_wiki_plan(out)?;
    finalize_agentic_plan(&mut plan)?;
    Ok(plan)
}

pub fn write_wiki(output_root: &Path, out: &WikiOutput<'_>) -> Result<()> {
    let plan = build_wiki_plan(out)?;
    write_plan_to_clean_dir(output_root, &plan)
}

/// 1 ページ分の追加。40,000 文字超は warn ログを出して処理は継続する（AC-23）。
fn write_page(plan: &mut OutputPlan, rel: &Path, body: &str) {
    let chars = body.chars().count();
    if chars > PAGE_CHAR_LIMIT {
        tracing::warn!(
            path = %markdown_path(rel),
            chars,
            limit = PAGE_CHAR_LIMIT,
            "page exceeds 128k-context soft limit"
        );
    }
    insert_text(plan, rel.to_path_buf(), body);
}
