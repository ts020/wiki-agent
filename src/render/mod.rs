pub mod index;
pub mod node;
pub mod paths;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::model::Node;

/// 出力ルートに wiki 一式を書き出す。既存の出力ディレクトリは毎回フル再生成する。
pub fn write_wiki(output_root: &Path, project_title: &str, nodes: &[Node]) -> Result<()> {
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

    for n in nodes {
        let path = output_root.join(&n.output_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let body = node::render_node(n);
        fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
    }

    let idx = index::render_index(project_title, nodes);
    fs::write(output_root.join("index.md"), idx)
        .with_context(|| format!("failed to write index.md under {}", output_root.display()))?;

    Ok(())
}
