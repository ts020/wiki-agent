use std::collections::HashMap;
use std::fmt::Write;
use std::path::PathBuf;

use super::paths::relative_link;
use crate::link::slugify;
use crate::model::Node;

/// `headings/index.md` の本文を生成する（FR-08）。
/// 全ノートの h1〜h2 を集約し、各見出しからノート内アンカーにリンクする。
pub fn render_headings_index(nodes: &[Node]) -> String {
    let mut s = String::new();
    let _ = writeln!(&mut s, "# Headings");
    s.push('\n');

    let from = PathBuf::from("headings/index.md");
    let mut any_heading = false;

    for n in nodes {
        let entries: Vec<(u8, String, String)> = slug_entries(&n.note.headings);
        if entries.is_empty() {
            continue;
        }
        any_heading = true;
        let link = relative_link(&from, &n.output_path);
        let _ = writeln!(&mut s, "## [{}]({})", n.title, link);
        s.push('\n');
        for (level, text, slug) in entries {
            let indent = if level == 2 { "  " } else { "" };
            let _ = writeln!(&mut s, "{indent}- [{text}]({link}#{slug})");
        }
        s.push('\n');
    }

    if !any_heading {
        let _ = writeln!(&mut s, "_(no headings)_");
    }
    s
}

/// h1〜h2 を抽出し、同一ページ内で衝突した slug に `-1`, `-2`, ... を付与する。
fn slug_entries(headings: &[crate::notes::Heading]) -> Vec<(u8, String, String)> {
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    for h in headings {
        if h.level != 1 && h.level != 2 {
            continue;
        }
        let base = slugify(&h.text);
        let slug = if let Some(count) = used.get_mut(&base) {
            *count += 1;
            format!("{base}-{count}")
        } else {
            used.insert(base.clone(), 0);
            base
        };
        out.push((h.level, h.text.clone(), slug));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::Heading;

    #[test]
    fn collides_slugs_get_suffix() {
        let hs = vec![
            Heading {
                level: 1,
                text: "Intro".into(),
            },
            Heading {
                level: 2,
                text: "Intro".into(),
            },
            Heading {
                level: 2,
                text: "Intro".into(),
            },
        ];
        let entries = slug_entries(&hs);
        assert_eq!(entries[0].2, "intro");
        assert_eq!(entries[1].2, "intro-1");
        assert_eq!(entries[2].2, "intro-2");
    }

    #[test]
    fn skips_h3_and_deeper() {
        let hs = vec![
            Heading {
                level: 1,
                text: "A".into(),
            },
            Heading {
                level: 3,
                text: "B".into(),
            },
            Heading {
                level: 2,
                text: "C".into(),
            },
        ];
        let entries = slug_entries(&hs);
        let levels: Vec<u8> = entries.iter().map(|e| e.0).collect();
        assert_eq!(levels, vec![1, 2]);
    }
}
