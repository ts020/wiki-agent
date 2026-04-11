use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::llm;
use crate::types::{KeyFile, WikiNode};

pub async fn generate(output_dir: &Path, nodes: &mut [WikiNode], root: &Path) -> Result<()> {
    let all_node_paths: Vec<String> = nodes
        .iter()
        .map(|n| n.output_path.display().to_string())
        .collect();

    for node in nodes.iter_mut() {
        let mut file_contents = String::new();
        for kf in &node.key_files {
            let abs_path = root.join(&kf.path);
            match std::fs::read_to_string(&abs_path) {
                Ok(content) => {
                    file_contents.push_str(&format!(
                        "--- {} ---\n{}\n\n",
                        kf.path.display(),
                        content
                    ));
                }
                Err(_) => continue,
            }
        }

        let sibling_nodes = all_node_paths
            .iter()
            .filter(|p| p.as_str() != node.output_path.display().to_string())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        eprintln!("生成中: {}", node.title);

        let content =
            llm::generate_node_content(&node.title, &file_contents, &sibling_nodes).await?;

        node.summary = content.summary;
        node.responsibilities = content.responsibilities;
        node.key_files = content
            .key_files
            .into_iter()
            .map(|kf| KeyFile {
                path: PathBuf::from(&kf.path),
                description: kf.description,
            })
            .collect();
    }

    for node in nodes.iter() {
        let out_path = output_dir.join(&node.output_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let markdown = render(node);
        std::fs::write(&out_path, markdown)?;
    }

    Ok(())
}

fn render(node: &WikiNode) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", node.title));

    out.push_str("## Summary\n\n");
    if node.summary.is_empty() {
        out.push_str("TODO\n\n");
    } else {
        out.push_str(&format!("{}\n\n", node.summary));
    }

    out.push_str("## Key files\n\n");
    if node.key_files.is_empty() {
        out.push_str("TODO\n\n");
    } else {
        for kf in &node.key_files {
            if kf.description.is_empty() {
                out.push_str(&format!("- `{}`\n", kf.path.display()));
            } else {
                out.push_str(&format!("- `{}` — {}\n", kf.path.display(), kf.description));
            }
        }
        out.push('\n');
    }

    out.push_str("## Responsibilities\n\n");
    if node.responsibilities.is_empty() {
        out.push_str("TODO\n\n");
    } else {
        for r in &node.responsibilities {
            out.push_str(&format!("- {r}\n"));
        }
        out.push('\n');
    }

    out.push_str("## Related\n\n");
    if node.related.is_empty() {
        out.push_str("None\n\n");
    } else {
        for r in &node.related {
            out.push_str(&format!("- [{}]({})\n", r.display(), r.display()));
        }
        out.push('\n');
    }

    out.push_str("## Read next\n\n");
    if node.read_next.is_empty() {
        out.push_str("None\n");
    } else {
        for r in &node.read_next {
            out.push_str(&format!("- [{}]({})\n", r.display(), r.display()));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::KeyFile;
    use std::path::PathBuf;

    #[test]
    fn render_contains_all_sections() {
        let node = WikiNode {
            title: "Test".to_string(),
            output_path: PathBuf::from("test.md"),
            summary: "A test node".to_string(),
            key_files: vec![KeyFile {
                path: PathBuf::from("src/main.rs"),
                description: "entry point".to_string(),
            }],
            responsibilities: vec!["handle requests".to_string()],
            related: vec![PathBuf::from("other.md")],
            read_next: vec![PathBuf::from("next.md")],
        };

        let md = render(&node);
        assert!(md.contains("# Test"));
        assert!(md.contains("## Summary"));
        assert!(md.contains("A test node"));
        assert!(md.contains("## Key files"));
        assert!(md.contains("`src/main.rs` — entry point"));
        assert!(md.contains("## Responsibilities"));
        assert!(md.contains("handle requests"));
        assert!(md.contains("## Related"));
        assert!(md.contains("other.md"));
        assert!(md.contains("## Read next"));
        assert!(md.contains("next.md"));
    }
}
