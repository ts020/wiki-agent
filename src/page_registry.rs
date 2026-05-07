use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::large_markdown::{PageKind, PagePlan};

pub const PAGE_HARD_LIMIT_CHARS: usize = 40_000;

#[derive(Debug, Default)]
pub struct PageRegistry {
    pages: BTreeMap<PathBuf, PagePlan>,
}

impl PageRegistry {
    pub fn build(plans: Vec<PagePlan>) -> Result<Self> {
        let mut pages = BTreeMap::new();
        for plan in plans {
            if plan.estimated_chars > PAGE_HARD_LIMIT_CHARS {
                bail!(
                    "page exceeds hard limit: {} chars at {}",
                    plan.estimated_chars,
                    plan.output_path.display()
                );
            }
            if pages.insert(plan.output_path.clone(), plan).is_some() {
                bail!("duplicate output path registered");
            }
        }
        let registry = Self { pages };
        registry.validate_navigation()?;
        Ok(registry)
    }

    pub fn pages(&self) -> impl Iterator<Item = &PagePlan> {
        self.pages.values()
    }

    fn validate_navigation(&self) -> Result<()> {
        let paths: BTreeSet<_> = self.pages.keys().cloned().collect();
        for plan in self.pages.values() {
            if let Some(parent) = &plan.parent {
                let target = resolve_neighbor(&plan.output_path, parent);
                if !paths.contains(&target) {
                    bail!(
                        "missing parent {} for {}",
                        parent.display(),
                        plan.output_path.display()
                    );
                }
            }
            if let Some(next) = &plan.next {
                let target = resolve_neighbor(&plan.output_path, next);
                let Some(next_plan) = self.pages.get(&target) else {
                    bail!(
                        "missing next {} for {}",
                        next.display(),
                        plan.output_path.display()
                    );
                };
                let back = next_plan
                    .prev
                    .as_ref()
                    .map(|prev| resolve_neighbor(&next_plan.output_path, prev));
                if back.as_ref() != Some(&plan.output_path) {
                    bail!(
                        "next.prev does not point back to {}",
                        plan.output_path.display()
                    );
                }
            }
        }
        Ok(())
    }
}

fn resolve_neighbor(from: &Path, rel: &Path) -> PathBuf {
    let base = from.parent().unwrap_or(Path::new(""));
    let mut out = PathBuf::new();
    for component in base.join(rel).components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => out.push(part),
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {}
        }
    }
    out
}

pub fn page_kind_name(kind: PageKind) -> &'static str {
    match kind {
        PageKind::Entry => "entry",
        PageKind::Shell => "shell",
        PageKind::Leaf => "leaf",
        PageKind::PagedIndex => "paged_index",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::large_markdown::{ByteRange, LineRange, SplitReason};

    fn plan(path: &str, kind: PageKind) -> PagePlan {
        PagePlan {
            page_id: path.into(),
            page_kind: kind,
            output_path: PathBuf::from(path),
            source_path: Some(PathBuf::from("source.md")),
            section_path: vec!["source.md".into()],
            byte_ranges: vec![ByteRange { start: 0, end: 1 }],
            line_ranges: vec![LineRange { start: 1, end: 1 }],
            split_reason: SplitReason::Heading,
            parent: None,
            prev: None,
            next: None,
            estimated_chars: 1,
        }
    }

    #[test]
    fn rejects_duplicate_path_missing_parent_and_oversize() {
        assert!(
            PageRegistry::build(vec![
                plan("fragments/a/index.md", PageKind::Entry),
                plan("fragments/a/index.md", PageKind::Leaf),
            ])
            .is_err()
        );

        let mut child = plan("fragments/a/part-001.md", PageKind::Leaf);
        child.parent = Some(PathBuf::from("index.md"));
        assert!(PageRegistry::build(vec![child]).is_err());

        let mut huge = plan("fragments/a/index.md", PageKind::Entry);
        huge.estimated_chars = PAGE_HARD_LIMIT_CHARS + 1;
        assert!(PageRegistry::build(vec![huge]).is_err());
    }

    #[test]
    fn validates_prev_next_relationships() {
        let entry = plan("fragments/a/index.md", PageKind::Entry);
        let mut first = plan("fragments/a/part-001.md", PageKind::Leaf);
        first.parent = Some(PathBuf::from("index.md"));
        first.next = Some(PathBuf::from("part-002.md"));
        let mut second = plan("fragments/a/part-002.md", PageKind::Leaf);
        second.parent = Some(PathBuf::from("index.md"));
        second.prev = Some(PathBuf::from("part-001.md"));
        assert!(PageRegistry::build(vec![entry, first, second]).is_ok());
    }
}
