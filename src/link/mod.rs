pub mod resolver;
pub mod slug;
pub mod wikilink;

pub use resolver::{Resolver, UnresolvedLink};
pub use slug::slugify;
pub use wikilink::{WikiLink, find_all};

use std::path::Path;

use crate::model::{Node, NodeKind};

/// すべてのノート由来ノードの本文中の wikilink を解決し、未解決一覧を返す。
pub fn resolve_all(nodes: &mut [Node]) -> Vec<UnresolvedLink> {
    let resolver = Resolver::build(nodes);
    let mut unresolved = Vec::new();
    for n in nodes.iter_mut() {
        if !matches!(n.kind, NodeKind::NoteDerived) {
            continue;
        }
        let Some(note) = n.note.as_mut() else {
            continue;
        };
        let (new_body, mut us) = wikilink::resolve_in(
            &note.body,
            &n.output_path,
            &resolver,
            &Path::new(&n.output_path)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        );
        note.body = new_body;
        unresolved.append(&mut us);
    }
    unresolved.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then_with(|| a.target.cmp(&b.target))
    });
    unresolved
}
