#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

/// Markdown 本文から ATX 見出し（`#` ... `######`）を抽出する。
/// 三連フェンス（``` / ~~~）の中は無視する。
pub fn extract(content: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut in_fenced = false;
    for raw in content.lines() {
        if is_fence(raw) {
            in_fenced = !in_fenced;
            continue;
        }
        if in_fenced {
            continue;
        }
        if let Some(h) = parse_atx(raw) {
            out.push(h);
        }
    }
    out
}

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

fn parse_atx(line: &str) -> Option<Heading> {
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
    Some(Heading { level, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_atx_headings() {
        let src = r#"# H1
some text
## H2-a
### H3
## H2-b"#;
        let hs = extract(src);
        assert_eq!(hs.len(), 4);
        assert_eq!(hs[0].level, 1);
        assert_eq!(hs[0].text, "H1");
        assert_eq!(hs[1].level, 2);
        assert_eq!(hs[1].text, "H2-a");
        assert_eq!(hs[2].level, 3);
        assert_eq!(hs[3].text, "H2-b");
    }

    #[test]
    fn skips_fenced_code_blocks() {
        let src = "```\n# not a heading\n```\n# real heading";
        let hs = extract(src);
        assert_eq!(hs.len(), 1);
        assert_eq!(hs[0].text, "real heading");
    }

    #[test]
    fn rejects_too_many_hashes_or_no_space() {
        assert!(parse_atx("####### too many").is_none());
        assert!(parse_atx("#no-space").is_none());
        assert_eq!(parse_atx("## OK").unwrap().text, "OK");
    }
}
