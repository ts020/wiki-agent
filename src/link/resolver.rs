use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use super::slug::slugify;
use crate::fragment::Fragment;
use crate::model::Node;
use crate::render::paths::{fragment_leaf_path, shell_index_path};

/// wikilink 解決のためのインデックス。すべて所有データなので借用問題を避ける。
#[derive(Debug, Default)]
pub struct Resolver {
    by_basename: BTreeMap<String, Vec<PathBuf>>,
    by_alias: BTreeMap<String, Vec<PathBuf>>,
    /// 入口ページ → 当該ノートの h2 解決表
    fragments_by_entry: BTreeMap<PathBuf, FragmentResolution>,
}

#[derive(Debug, Clone, Default)]
struct FragmentResolution {
    non_fragmented: bool,
    /// 断片化時: h2 slug → 出力先（断片ページ or 殻 index.md）
    h2_pages: BTreeMap<String, PathBuf>,
    /// 非断片化時: 入口ページ内で参照可能な h2 slug 集合
    non_frag_h2_slugs: BTreeSet<String>,
}

/// wikilink の解決結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// ノートのみ参照（`[[Foo]]`）。入口ページを指す。
    Entry(PathBuf),
    /// 非断片化ノート内のアンカー（`[[Foo#sec]]`）。`(入口ページ, slug)`。
    EntryAnchor(PathBuf, String),
    /// 断片ページを直接指すリンク（`[[Foo#h2]]`）。
    Page(PathBuf),
    /// 解決失敗。
    Missing,
}

impl Resolver {
    pub fn build(nodes: &[Node]) -> Self {
        let mut r = Self::default();
        for n in nodes {
            let target = n.output_path.clone();
            if let Some(stem) = n.note.source_file.file_stem().and_then(|s| s.to_str()) {
                r.by_basename
                    .entry(stem.to_string())
                    .or_default()
                    .push(target.clone());
            }
            for alias in &n.note.frontmatter.aliases {
                r.by_alias
                    .entry(alias.clone())
                    .or_default()
                    .push(target.clone());
            }
            if let Some(title) = &n.note.frontmatter.title {
                r.by_alias
                    .entry(title.clone())
                    .or_default()
                    .push(target.clone());
            }
            r.fragments_by_entry
                .insert(target, build_fragment_resolution(n));
        }
        r
    }

    /// 入口ページへ解決する（後方互換用：`#heading` が無い場合と同等）。
    /// `from_entry` は参照元ノートの入口ページ出力相対パス。
    pub fn resolve(&self, name: &str, from_entry: &Path) -> Option<PathBuf> {
        if let Some(list) = self.by_alias.get(name)
            && let Some(p) = pick_best(list, from_entry)
        {
            return Some(p);
        }
        if let Some(list) = self.by_basename.get(name)
            && let Some(p) = pick_best(list, from_entry)
        {
            return Some(p);
        }
        None
    }

    /// 名前 + オプションの見出しアンカーを解決する（FR-06）。
    pub fn resolve_with_heading(
        &self,
        name: &str,
        heading: Option<&str>,
        from_entry: &Path,
    ) -> Resolution {
        let Some(entry) = self.resolve(name, from_entry) else {
            return Resolution::Missing;
        };
        let Some(h) = heading else {
            return Resolution::Entry(entry);
        };
        let slug = slugify(h);
        let Some(fr) = self.fragments_by_entry.get(&entry) else {
            return Resolution::Missing;
        };
        if fr.non_fragmented {
            if fr.non_frag_h2_slugs.contains(&slug) {
                Resolution::EntryAnchor(entry, slug)
            } else {
                Resolution::Missing
            }
        } else if let Some(page) = fr.h2_pages.get(&slug) {
            Resolution::Page(page.clone())
        } else {
            Resolution::Missing
        }
    }
}

fn build_fragment_resolution(n: &Node) -> FragmentResolution {
    let mut res = FragmentResolution {
        non_fragmented: n.fragments.non_fragmented,
        ..Default::default()
    };
    if n.fragments.non_fragmented {
        let mut used: HashMap<String, usize> = HashMap::new();
        for h in &n.note.headings {
            if h.level != 2 {
                continue;
            }
            let base = slugify(&h.text);
            let slug = disambiguate(&base, &mut used);
            res.non_frag_h2_slugs.insert(slug);
        }
    } else {
        for frag in &n.fragments.fragments {
            match frag {
                Fragment::H2 { slug, .. } => {
                    res.h2_pages
                        .insert(slug.clone(), fragment_leaf_path(&n.entry_dir, slug));
                }
                Fragment::Shell { slug, .. } => {
                    res.h2_pages
                        .insert(slug.clone(), shell_index_path(&n.entry_dir, slug));
                }
            }
        }
    }
    res
}

fn disambiguate(base: &str, used: &mut HashMap<String, usize>) -> String {
    if let Some(count) = used.get_mut(base) {
        *count += 1;
        format!("{base}-{count}")
    } else {
        used.insert(base.to_string(), 0);
        base.to_string()
    }
}

/// `fragments/<rel>/index.md` 型のパスから、「ノートが置かれた元ディレクトリ」に
/// 相当する親（入口ディレクトリの親）を取り出す。
fn source_dir_of(entry_path: &Path) -> &Path {
    entry_path
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new(""))
}

fn pick_best(paths: &[PathBuf], from_entry: &Path) -> Option<PathBuf> {
    if paths.is_empty() {
        return None;
    }
    if paths.len() == 1 {
        return Some(paths[0].clone());
    }
    let from_dir = source_dir_of(from_entry);
    for p in paths {
        if source_dir_of(p) == from_dir {
            return Some(p.clone());
        }
    }
    let mut sorted = paths.to_vec();
    sorted.sort_by(|a, b| {
        a.components()
            .count()
            .cmp(&b.components().count())
            .then_with(|| a.cmp(b))
    });
    sorted.into_iter().next()
}

#[derive(Debug, Clone)]
pub struct UnresolvedLink {
    /// 参照元ページの出力相対パス
    pub source: PathBuf,
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::{Frontmatter, Heading, NoteData};

    fn node(src: &str) -> Node {
        let source = PathBuf::from(src);
        let entry = crate::render::paths::entry_index_path(&source);
        let entry_dir = entry.parent().unwrap().to_path_buf();
        Node {
            output_path: entry,
            entry_dir,
            title: "t".into(),
            note: NoteData {
                source_file: source,
                frontmatter: Frontmatter::default(),
                headings: vec![],
                first_paragraph: None,
                body: String::new(),
            },
            fragments: crate::fragment::FragmentTree::default(),
            related: vec![],
            backlinks: std::collections::BTreeMap::new(),
        }
    }

    fn node_with_fragments(src: &str, body: &str) -> Node {
        let mut n = node(src);
        n.note.body = body.to_string();
        n.note.headings = crate::notes::headings::extract(body);
        n.fragments = crate::fragment::build_fragments(&n.note);
        n
    }

    #[test]
    fn resolves_by_basename() {
        let nodes = vec![node("foo.md"), node("docs/bar.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("fragments/other/index.md")),
            Some(PathBuf::from("fragments/foo/index.md"))
        );
    }

    #[test]
    fn same_directory_wins_on_collision() {
        let nodes = vec![node("a/foo.md"), node("b/foo.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("fragments/a/bar/index.md")),
            Some(PathBuf::from("fragments/a/foo/index.md"))
        );
    }

    #[test]
    fn shortest_path_then_alpha_wins() {
        let nodes = vec![node("deep/deep/foo.md"), node("foo.md")];
        let r = Resolver::build(&nodes);
        assert_eq!(
            r.resolve("foo", Path::new("fragments/other/here/index.md")),
            Some(PathBuf::from("fragments/foo/index.md"))
        );
    }

    #[test]
    fn returns_none_when_unknown() {
        let nodes = vec![node("foo.md")];
        let r = Resolver::build(&nodes);
        assert!(
            r.resolve("bar", Path::new("fragments/x/index.md"))
                .is_none()
        );
    }

    #[test]
    fn heading_resolves_to_fragment_page() {
        let nodes = vec![node_with_fragments(
            "foo.md",
            "# Foo\n\n## Intro\n\nhi\n\n## Design\n\nx\n",
        )];
        let r = Resolver::build(&nodes);
        let from = Path::new("fragments/other/index.md");
        let res = r.resolve_with_heading("foo", Some("Intro"), from);
        assert_eq!(
            res,
            Resolution::Page(PathBuf::from("fragments/foo/intro.md"))
        );
    }

    #[test]
    fn missing_heading_is_unresolved() {
        let nodes = vec![node_with_fragments("foo.md", "# Foo\n\n## Intro\n")];
        let r = Resolver::build(&nodes);
        let from = Path::new("fragments/other/index.md");
        let res = r.resolve_with_heading("foo", Some("Nope"), from);
        assert_eq!(res, Resolution::Missing);
    }

    #[test]
    fn heading_on_non_fragmented_note_uses_anchor() {
        // fragment:false で強制非断片化
        let source = PathBuf::from("foo.md");
        let mut note = NoteData {
            source_file: source.clone(),
            frontmatter: Frontmatter {
                fragment: Some(false),
                ..Default::default()
            },
            headings: vec![
                Heading {
                    level: 1,
                    text: "Foo".into(),
                },
                Heading {
                    level: 2,
                    text: "Design".into(),
                },
            ],
            first_paragraph: None,
            body: "# Foo\n\n## Design\n".into(),
        };
        let entry = crate::render::paths::entry_index_path(&source);
        note.headings = crate::notes::headings::extract(&note.body);
        let fragments = crate::fragment::build_fragments(&note);
        let n = Node {
            output_path: entry.clone(),
            entry_dir: entry.parent().unwrap().to_path_buf(),
            title: "Foo".into(),
            note,
            fragments,
            related: vec![],
            backlinks: std::collections::BTreeMap::new(),
        };
        let r = Resolver::build(&[n]);
        let from = Path::new("fragments/other/index.md");
        let res = r.resolve_with_heading("foo", Some("Design"), from);
        assert_eq!(
            res,
            Resolution::EntryAnchor(PathBuf::from("fragments/foo/index.md"), "design".into())
        );
    }
}
