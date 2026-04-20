pub mod development;
pub mod index;
pub mod node;
pub mod overview;
pub mod paths;
pub mod unresolved;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::extract::{EntryPoint, TechStack, TestLayout};
use crate::link::UnresolvedLink;
use crate::model::Node;

/// 生成物一式を出力ルートへ書き出す。毎回フル再生成（FR-03）。
pub struct WikiOutput<'a> {
    pub project_title: &'a str,
    pub nodes: &'a [Node],
    pub tech_stack: &'a TechStack,
    pub entry_points: &'a [EntryPoint],
    pub test_layout: &'a TestLayout,
    pub unresolved: &'a [UnresolvedLink],
}

pub fn write_wiki(output_root: &Path, out: &WikiOutput<'_>) -> Result<()> {
    if output_root.exists() {
        if !output_root.is_dir() {
            anyhow::bail!(
                "output path exists but is not a directory: {}",
                output_root.display()
            );
        }
        fs::remove_dir_all(output_root)
            .with_context(|| format!("failed to clear {}", output_root.display()))?;
    }
    fs::create_dir_all(output_root)
        .with_context(|| format!("failed to create {}", output_root.display()))?;

    for n in out.nodes {
        let path = output_root.join(&n.output_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, node::render_node(n))
            .with_context(|| format!("failed to write {}", path.display()))?;

        if let Some(op) = &n.symbols_overflow_path {
            let abs = output_root.join(op);
            if let Some(parent) = abs.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            fs::write(&abs, node::render_symbols_overflow(n))
                .with_context(|| format!("failed to write {}", abs.display()))?;
        }
    }

    write_file(
        output_root,
        "overview/tech-stack.md",
        &overview::render_tech_stack(out.tech_stack),
    )?;
    write_file(
        output_root,
        "overview/entry-points.md",
        &overview::render_entry_points(out.entry_points),
    )?;
    write_file(
        output_root,
        "overview/tests.md",
        &overview::render_tests(out.test_layout),
    )?;
    write_file(
        output_root,
        "development/index.md",
        &development::render_development(out.tech_stack),
    )?;

    write_file(
        output_root,
        "_unresolved.md",
        &unresolved::render_unresolved(out.unresolved),
    )?;

    let idx = index::render_index(
        out.project_title,
        out.nodes,
        out.tech_stack,
        out.entry_points,
        out.test_layout,
        out.unresolved,
    );
    fs::write(output_root.join("index.md"), idx)
        .with_context(|| format!("failed to write index.md under {}", output_root.display()))?;

    Ok(())
}

fn write_file(root: &Path, rel: &str, body: &str) -> Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
