use std::path::PathBuf;

use crate::fragment::Fragment;
use crate::large_markdown::{PageKind, PagePlan, SplitReason};
use crate::model::Node;
use crate::render::paths::{fragment_leaf_path, h3_leaf_path, shell_index_path};

pub fn plan_regular_pages(nodes: &[Node]) -> Vec<PagePlan> {
    let mut plans = Vec::new();
    for node in nodes {
        plans.extend(plan_node(node));
    }
    plans
}

fn plan_node(node: &Node) -> Vec<PagePlan> {
    let source = Some(node.note.source_file.clone());
    let mut out = vec![PagePlan {
        page_id: format!("{}:entry", node.output_path.display()),
        page_kind: PageKind::Entry,
        output_path: node.output_path.clone(),
        source_path: source.clone(),
        section_path: vec![node.title.clone()],
        byte_ranges: Vec::new(),
        line_ranges: Vec::new(),
        split_reason: SplitReason::Heading,
        parent: None,
        prev: None,
        next: None,
        estimated_chars: node.fragments.preface.chars().count(),
    }];

    for (idx, fragment) in node.fragments.fragments.iter().enumerate() {
        match fragment {
            Fragment::H2 {
                slug,
                heading,
                body,
            } => {
                let prev = (idx > 0).then(|| relative_fragment_target(idx - 1, node));
                let next = (idx + 1 < node.fragments.fragments.len())
                    .then(|| relative_fragment_target(idx + 1, node));
                out.push(PagePlan {
                    page_id: format!("{}:{slug}", node.output_path.display()),
                    page_kind: PageKind::Leaf,
                    output_path: fragment_leaf_path(&node.entry_dir, slug),
                    source_path: source.clone(),
                    section_path: vec![node.title.clone(), heading.clone()],
                    byte_ranges: Vec::new(),
                    line_ranges: Vec::new(),
                    split_reason: SplitReason::Heading,
                    parent: Some(PathBuf::from("index.md")),
                    prev,
                    next,
                    estimated_chars: body.chars().count(),
                });
            }
            Fragment::Shell {
                slug,
                heading,
                preface,
                children,
            } => {
                out.push(PagePlan {
                    page_id: format!("{}:{slug}:shell", node.output_path.display()),
                    page_kind: PageKind::Shell,
                    output_path: shell_index_path(&node.entry_dir, slug),
                    source_path: source.clone(),
                    section_path: vec![node.title.clone(), heading.clone()],
                    byte_ranges: Vec::new(),
                    line_ranges: Vec::new(),
                    split_reason: SplitReason::Heading,
                    parent: Some(PathBuf::from("index.md")),
                    prev: None,
                    next: None,
                    estimated_chars: preface.chars().count(),
                });
                for (child_idx, child) in children.iter().enumerate() {
                    out.push(PagePlan {
                        page_id: format!("{}:{slug}:{}", node.output_path.display(), child.slug),
                        page_kind: PageKind::Leaf,
                        output_path: h3_leaf_path(&node.entry_dir, slug, &child.slug),
                        source_path: source.clone(),
                        section_path: vec![
                            node.title.clone(),
                            heading.clone(),
                            child.heading.clone(),
                        ],
                        byte_ranges: Vec::new(),
                        line_ranges: Vec::new(),
                        split_reason: SplitReason::Heading,
                        parent: Some(PathBuf::from("index.md")),
                        prev: (child_idx > 0)
                            .then(|| PathBuf::from(format!("{}.md", children[child_idx - 1].slug))),
                        next: (child_idx + 1 < children.len())
                            .then(|| PathBuf::from(format!("{}.md", children[child_idx + 1].slug))),
                        estimated_chars: child.body.chars().count(),
                    });
                }
            }
        }
    }
    out
}

fn relative_fragment_target(idx: usize, node: &Node) -> PathBuf {
    match &node.fragments.fragments[idx] {
        Fragment::H2 { slug, .. } => PathBuf::from(format!("{slug}.md")),
        Fragment::Shell { slug, .. } => PathBuf::from(slug).join("index.md"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::build_nodes;
    use crate::notes::{Frontmatter, NoteData, headings};

    fn node(body: &str) -> Node {
        build_nodes(vec![NoteData {
            source_file: PathBuf::from("n.md"),
            frontmatter: Frontmatter::default(),
            headings: headings::extract(body),
            first_paragraph: None,
            body: body.into(),
        }])
        .remove(0)
    }

    #[test]
    fn h2_note_has_entry_and_leaf_with_navigation() {
        let plans = plan_regular_pages(&[node("# N\n\n## A\n\na\n")]);
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].page_kind, PageKind::Entry);
        assert_eq!(plans[1].page_kind, PageKind::Leaf);
        assert_eq!(
            plans[1].parent.as_deref(),
            Some(std::path::Path::new("index.md"))
        );
    }

    #[test]
    fn multiple_h2_pages_have_prev_next() {
        let plans = plan_regular_pages(&[node("# N\n\n## A\n\na\n\n## B\n\nb\n")]);
        let leaves: Vec<_> = plans
            .iter()
            .filter(|plan| plan.page_kind == PageKind::Leaf)
            .collect();
        assert_eq!(
            leaves[0].next.as_deref(),
            Some(std::path::Path::new("b.md"))
        );
        assert_eq!(
            leaves[1].prev.as_deref(),
            Some(std::path::Path::new("a.md"))
        );
    }
}
