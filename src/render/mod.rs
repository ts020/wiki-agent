pub mod index;
pub mod node;
pub mod paths;
pub mod tags;
pub mod unresolved;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::link::{Resolver, UnresolvedLink};
use crate::model::Node;

/// 生成物一式を出力ルートへ書き出す。毎回フル再生成（FR-03）。
pub struct WikiOutput<'a> {
    pub project_title: &'a str,
    pub nodes: &'a [Node],
    pub unresolved: &'a [UnresolvedLink],
}

pub fn write_wiki(output_root: &Path, out: &WikiOutput<'_>) -> Result<()> {
    clean_output(output_root)?;
    fs::create_dir_all(output_root)
        .with_context(|| format!("failed to create {}", output_root.display()))?;

    let titles: std::collections::BTreeMap<std::path::PathBuf, String> = out
        .nodes
        .iter()
        .map(|n| (n.output_path.clone(), n.title.clone()))
        .collect();

    let resolver = Resolver::build(out.nodes);

    for n in out.nodes {
        let path = output_root.join(&n.output_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, node::render_node(n, &titles, &resolver))
            .with_context(|| format!("failed to write {}", path.display()))?;
    }

    // タグ索引
    let tag_index = tags::build_tag_index(out.nodes);
    for (tag, paths) in &tag_index.entries {
        let path = tags::tag_page_path(tag);
        let body = tags::render_tag_page(tag, paths, out.nodes);
        let abs = output_root.join(&path);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&abs, body).with_context(|| format!("failed to write {}", abs.display()))?;
    }

    write_file(
        output_root,
        "_unresolved.md",
        &unresolved::render_unresolved(out.unresolved),
    )?;

    let idx = index::render_index(out.project_title, out.nodes, out.unresolved, &tag_index);
    fs::write(output_root.join("index.md"), idx)
        .with_context(|| format!("failed to write index.md under {}", output_root.display()))?;

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
        output_root.join("index.md").exists() || output_root.join("notes").exists();
    if !looks_like_ours {
        anyhow::bail!(
            "refusing to clean {}: does not look like a md-wiki output directory \
             (no index.md or notes/). Remove it manually or choose a different --out.",
            output_root.display()
        );
    }

    fs::remove_dir_all(output_root)
        .with_context(|| format!("failed to clear {}", output_root.display()))?;
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
