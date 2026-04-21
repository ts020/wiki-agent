//! ノート本体を h2（必要なら h3）単位で断片化する純粋関数（FR-05）。
//!
//! 入力 `NoteData` から `FragmentTree` を組み立てる。I/O を伴わないのでユニットテストしやすい。
//! 出力パスの決定や本文への wikilink 変換はここでは行わず、上位（`build`/`render`）に任せる。

use std::collections::HashSet;

use crate::link::slug::slugify;
use crate::notes::NoteData;

/// h3 再分割を発動する本文行数の閾値（FR-05 §5.3）。
/// 本文長が**超えた**（> 300）ときに殻化する（= 300 は殻化しない）。
const H3_RESPLIT_THRESHOLD: usize = 300;

/// ノート 1 枚の断片化結果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FragmentTree {
    /// 入口ページに載せる本文。
    /// 断片化時はプレフェイス（最初の h2 直前まで）、非断片化時はノート全文。
    pub preface: String,
    /// 直下断片（h2 単位）。非断片化時は空。
    pub fragments: Vec<Fragment>,
    /// h2 が無い／`fragment: false` により断片化を行わなかった場合 true。
    pub non_fragmented: bool,
}

/// h2 粒度の断片。通常断片と殻の 2 種類。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fragment {
    /// h2 通常断片。body には h2 見出し行から次の h2 直前までを含む。
    H2 {
        heading: String,
        slug: String,
        body: String,
    },
    /// h3 再分割された h2（殻ページに降格）。
    Shell {
        heading: String,
        slug: String,
        /// h2 見出し行から最初の h3 直前までの本文（殻ページにそのまま置く）。
        preface: String,
        children: Vec<H3Fragment>,
    },
}

/// h3 粒度の子断片。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H3Fragment {
    pub heading: String,
    pub slug: String,
    /// h3 見出し行を含む本文（次の h3 の直前、または親 h2 の終端まで）。
    pub body: String,
}

impl Fragment {
    pub fn heading(&self) -> &str {
        match self {
            Fragment::H2 { heading, .. } | Fragment::Shell { heading, .. } => heading,
        }
    }

    pub fn slug(&self) -> &str {
        match self {
            Fragment::H2 { slug, .. } | Fragment::Shell { slug, .. } => slug,
        }
    }
}

/// ノート 1 枚の本体を断片化する。
pub fn build_fragments(note: &NoteData) -> FragmentTree {
    if note.frontmatter.fragment == Some(false) {
        return non_fragmented(&note.body);
    }
    let markers = scan_markers(&note.body);
    let h2_markers: Vec<&Marker> = markers.iter().filter(|m| m.level == 2).collect();
    if h2_markers.is_empty() {
        return non_fragmented(&note.body);
    }

    let lines: Vec<&str> = note.body.split('\n').collect();
    let preface = join_slice(&lines, 0, h2_markers[0].line_idx);

    let raw = collect_raw_fragments(&lines, &markers, &h2_markers);
    let fragments = assign_slugs(raw);

    FragmentTree {
        preface,
        fragments,
        non_fragmented: false,
    }
}

fn non_fragmented(body: &str) -> FragmentTree {
    FragmentTree {
        preface: body.to_string(),
        fragments: Vec::new(),
        non_fragmented: true,
    }
}

#[derive(Debug)]
struct Marker {
    line_idx: usize,
    level: u8,
    text: String,
}

fn scan_markers(body: &str) -> Vec<Marker> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (i, line) in body.split('\n').enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some((level, text)) = parse_atx(line)
            && (2..=3).contains(&level)
        {
            out.push(Marker {
                line_idx: i,
                level,
                text,
            });
        }
    }
    out
}

fn parse_atx(line: &str) -> Option<(u8, String)> {
    let t = line.trim_start();
    let mut level: u8 = 0;
    for c in t.chars() {
        if c == '#' {
            level += 1;
            if level > 6 {
                return None;
            }
        } else {
            break;
        }
    }
    if level == 0 {
        return None;
    }
    let rest = &t[level as usize..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim().to_string();
    Some((level, text))
}

enum RawFragment {
    H2 {
        heading: String,
        body: String,
    },
    Shell {
        heading: String,
        preface: String,
        children: Vec<RawH3>,
    },
}

struct RawH3 {
    heading: String,
    body: String,
}

fn collect_raw_fragments(
    lines: &[&str],
    markers: &[Marker],
    h2_markers: &[&Marker],
) -> Vec<RawFragment> {
    let mut out = Vec::with_capacity(h2_markers.len());
    for (i, m) in h2_markers.iter().enumerate() {
        let start = m.line_idx;
        let end = h2_markers
            .get(i + 1)
            .map(|n| n.line_idx)
            .unwrap_or(lines.len());
        // 本文長: h2 見出しの直後から次の h2 の直前まで（h2 行を含めない）
        let body_line_count = end.saturating_sub(start).saturating_sub(1);
        let h3_in_range: Vec<&Marker> = markers
            .iter()
            .filter(|mk| mk.level == 3 && mk.line_idx > start && mk.line_idx < end)
            .collect();

        if body_line_count > H3_RESPLIT_THRESHOLD && h3_in_range.len() >= 2 {
            let first_h3_idx = h3_in_range[0].line_idx;
            let shell_preface = join_slice(lines, start, first_h3_idx);
            let mut children = Vec::with_capacity(h3_in_range.len());
            for (j, h3) in h3_in_range.iter().enumerate() {
                let h3_start = h3.line_idx;
                let h3_end = h3_in_range.get(j + 1).map(|n| n.line_idx).unwrap_or(end);
                children.push(RawH3 {
                    heading: h3.text.clone(),
                    body: join_slice(lines, h3_start, h3_end),
                });
            }
            out.push(RawFragment::Shell {
                heading: m.text.clone(),
                preface: shell_preface,
                children,
            });
        } else {
            out.push(RawFragment::H2 {
                heading: m.text.clone(),
                body: join_slice(lines, start, end),
            });
        }
    }
    out
}

fn join_slice(lines: &[&str], start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }
    lines[start..end].join("\n")
}

fn assign_slugs(raw: Vec<RawFragment>) -> Vec<Fragment> {
    let mut used: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(raw.len());
    for item in raw {
        match item {
            RawFragment::H2 { heading, body } => {
                let slug = dedup_slug(&slug_for_heading(&heading), &mut used);
                out.push(Fragment::H2 {
                    heading,
                    slug,
                    body,
                });
            }
            RawFragment::Shell {
                heading,
                preface,
                children,
            } => {
                let slug = dedup_slug(&slug_for_heading(&heading), &mut used);
                let mut child_used: HashSet<String> = HashSet::new();
                let children = children
                    .into_iter()
                    .map(|c| {
                        let cslug = dedup_slug(&slug_for_heading(&c.heading), &mut child_used);
                        H3Fragment {
                            heading: c.heading,
                            slug: cslug,
                            body: c.body,
                        }
                    })
                    .collect();
                out.push(Fragment::Shell {
                    heading,
                    slug,
                    preface,
                    children,
                });
            }
        }
    }
    out
}

fn slug_for_heading(heading: &str) -> String {
    let s = slugify(heading);
    if s.is_empty() { "section".into() } else { s }
}

fn dedup_slug(base: &str, used: &mut HashSet<String>) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    let mut n = 1;
    loop {
        let cand = format!("{base}-{n}");
        if used.insert(cand.clone()) {
            return cand;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::{Frontmatter, NoteData};
    use std::path::PathBuf;

    fn note_with(body: &str, fm: Frontmatter) -> NoteData {
        NoteData {
            source_file: PathBuf::from("x.md"),
            frontmatter: fm,
            headings: Vec::new(),
            first_paragraph: None,
            body: body.to_string(),
        }
    }

    fn note(body: &str) -> NoteData {
        note_with(body, Frontmatter::default())
    }

    #[test]
    fn splits_into_h2_fragments_with_preface() {
        let body = "# Title\n\nintro\n\n## A\n\na body\n\n## B\n\nb body\n";
        let tree = build_fragments(&note(body));
        assert!(!tree.non_fragmented);
        assert_eq!(tree.preface, "# Title\n\nintro\n");
        assert_eq!(tree.fragments.len(), 2);
        assert_eq!(tree.fragments[0].heading(), "A");
        assert_eq!(tree.fragments[0].slug(), "a");
        match &tree.fragments[0] {
            Fragment::H2 { body, .. } => assert!(body.starts_with("## A")),
            _ => panic!("expected H2"),
        }
    }

    #[test]
    fn single_h2_still_splits() {
        let body = "# T\n\npre\n\n## Only\n\nhello\n";
        let tree = build_fragments(&note(body));
        assert!(!tree.non_fragmented);
        assert_eq!(tree.fragments.len(), 1);
        assert_eq!(tree.fragments[0].heading(), "Only");
        assert_eq!(tree.preface, "# T\n\npre\n");
    }

    #[test]
    fn no_h2_is_non_fragmented() {
        let body = "# T\n\nbody\n\n### sub\n\ndeep\n";
        let tree = build_fragments(&note(body));
        assert!(tree.non_fragmented);
        assert!(tree.fragments.is_empty());
        assert_eq!(tree.preface, body);
    }

    #[test]
    fn fragment_false_forces_non_fragmented() {
        let body = "## A\n\na\n\n## B\n\nb\n";
        let fm = Frontmatter {
            fragment: Some(false),
            ..Default::default()
        };
        let tree = build_fragments(&note_with(body, fm));
        assert!(tree.non_fragmented);
        assert!(tree.fragments.is_empty());
        assert_eq!(tree.preface, body);
    }

    #[test]
    fn slug_collision_gets_suffixed() {
        let body = "## Same\n\nx\n\n## Same\n\ny\n";
        let tree = build_fragments(&note(body));
        assert_eq!(tree.fragments.len(), 2);
        assert_eq!(tree.fragments[0].slug(), "same");
        assert_eq!(tree.fragments[1].slug(), "same-1");
    }

    #[test]
    fn headings_inside_fenced_code_are_ignored() {
        let body = "## Real\n\n```\n## fake in code\n```\n\nbody\n\n## Next\n\nn\n";
        let tree = build_fragments(&note(body));
        assert_eq!(tree.fragments.len(), 2);
        assert_eq!(tree.fragments[0].heading(), "Real");
        assert_eq!(tree.fragments[1].heading(), "Next");
    }

    #[test]
    fn h3_resplit_triggers_when_hard_limit_exceeded_and_two_h3s() {
        // 300 行超かつ h3 が 2 個以上: 殻化
        let mut body = String::from("## Big\n\n");
        body.push_str("### alpha\n");
        // 150 行のダミー
        for i in 0..150 {
            body.push_str(&format!("line{i}\n"));
        }
        body.push_str("### bravo\n");
        for i in 0..200 {
            body.push_str(&format!("more{i}\n"));
        }
        let tree = build_fragments(&note(&body));
        assert_eq!(tree.fragments.len(), 1);
        match &tree.fragments[0] {
            Fragment::Shell {
                heading,
                slug,
                preface,
                children,
            } => {
                assert_eq!(heading, "Big");
                assert_eq!(slug, "big");
                assert_eq!(preface, "## Big\n");
                assert_eq!(children.len(), 2);
                assert_eq!(children[0].heading, "alpha");
                assert_eq!(children[0].slug, "alpha");
                assert!(children[0].body.starts_with("### alpha"));
                assert_eq!(children[1].heading, "bravo");
            }
            _ => panic!("expected Shell"),
        }
    }

    #[test]
    fn h3_resplit_requires_two_or_more_h3s() {
        // 300 行超でも h3 が 1 個なら殻化しない
        let mut body = String::from("## Big\n\n### only\n");
        for i in 0..400 {
            body.push_str(&format!("line{i}\n"));
        }
        let tree = build_fragments(&note(&body));
        assert_eq!(tree.fragments.len(), 1);
        assert!(matches!(&tree.fragments[0], Fragment::H2 { .. }));
    }

    #[test]
    fn h3_resplit_respects_hard_threshold_boundary() {
        // h2 本文長ちょうど 300 行なら殻化しない
        fn body_with_n_lines_after_h2(n: usize) -> String {
            // split('\n') 後に h2 行(1) + n 行を得たい。
            // 内訳: "### a" + (n - 2) 行の "x" + "### b"（計 n 行）
            assert!(n >= 2);
            let mut s = String::from("## Big\n### a");
            for _ in 0..(n - 2) {
                s.push_str("\nx");
            }
            s.push_str("\n### b");
            s
        }
        // n = 300: 殻化しない
        let body = body_with_n_lines_after_h2(300);
        let tree = build_fragments(&note(&body));
        assert_eq!(tree.fragments.len(), 1);
        assert!(
            matches!(&tree.fragments[0], Fragment::H2 { .. }),
            "300 行ちょうどでは殻化しないこと"
        );
        // n = 301: 殻化する
        let body = body_with_n_lines_after_h2(301);
        let tree = build_fragments(&note(&body));
        assert!(matches!(&tree.fragments[0], Fragment::Shell { .. }));
    }

    #[test]
    fn shell_child_slugs_are_scoped_per_shell() {
        // 別の h2 配下の同名 h3 とは衝突しない
        let mut body = String::from("## One\n");
        body.push_str("### dup\n");
        for i in 0..200 {
            body.push_str(&format!("a{i}\n"));
        }
        body.push_str("### dup\n");
        for i in 0..200 {
            body.push_str(&format!("b{i}\n"));
        }
        body.push_str("## Two\n");
        body.push_str("### dup\n");
        for i in 0..200 {
            body.push_str(&format!("c{i}\n"));
        }
        body.push_str("### dup\n");
        for i in 0..200 {
            body.push_str(&format!("d{i}\n"));
        }
        let tree = build_fragments(&note(&body));
        assert_eq!(tree.fragments.len(), 2);
        match &tree.fragments[0] {
            Fragment::Shell { children, .. } => {
                assert_eq!(children[0].slug, "dup");
                assert_eq!(children[1].slug, "dup-1");
            }
            _ => panic!("expected Shell"),
        }
        match &tree.fragments[1] {
            Fragment::Shell { children, .. } => {
                assert_eq!(children[0].slug, "dup"); // 別 shell では 0 から始まる
                assert_eq!(children[1].slug, "dup-1");
            }
            _ => panic!("expected Shell"),
        }
    }

    #[test]
    fn shell_slug_and_leaf_slug_share_scope() {
        // 殻 h2 と通常 h2 が同 slug のときは出現順で -1 付与
        let mut body = String::from("## Same\n");
        // 殻化しない通常断片
        body.push_str("a\n");
        body.push_str("## Same\n");
        // 殻化: 300 行超 + h3 2 個
        body.push_str("### x\n");
        for i in 0..200 {
            body.push_str(&format!("{i}\n"));
        }
        body.push_str("### y\n");
        for i in 0..200 {
            body.push_str(&format!("y{i}\n"));
        }
        let tree = build_fragments(&note(&body));
        assert_eq!(tree.fragments.len(), 2);
        assert_eq!(tree.fragments[0].slug(), "same");
        assert_eq!(tree.fragments[1].slug(), "same-1");
    }

    #[test]
    fn empty_heading_falls_back_to_section_slug() {
        let body = "## \n\nbody\n";
        let tree = build_fragments(&note(body));
        assert_eq!(tree.fragments.len(), 1);
        assert_eq!(tree.fragments[0].slug(), "section");
    }
}
