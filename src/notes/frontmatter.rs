#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontmatter {
    pub wiki: Option<bool>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub related: Vec<String>,
    pub aliases: Vec<String>,
}

/// 先頭の YAML フロントマターを抽出し、本文と分離する。
///
/// 仕様:
/// - ファイル先頭が `---\n` で始まる場合のみフロントマターとして扱う
/// - 独立行の `---` が閉じ
/// - 見つからない場合は `(None, content)`
pub fn split(content: &str) -> (Option<Frontmatter>, String) {
    let rest = match content.strip_prefix("---\n") {
        Some(r) => r,
        None => return (None, content.to_string()),
    };
    let Some(end_pos) = find_end_delim(rest) else {
        return (None, content.to_string());
    };
    let yaml_block = &rest[..end_pos];
    let body_start = rest[end_pos..]
        .find('\n')
        .map(|n| end_pos + n + 1)
        .unwrap_or(rest.len());
    let body = rest[body_start..].to_string();
    (Some(parse(yaml_block)), body)
}

fn find_end_delim(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut search_from = 0;
    while let Some(pos) = s[search_from..].find("---") {
        let abs = search_from + pos;
        let at_line_start = abs == 0 || bytes[abs - 1] == b'\n';
        let after = abs + 3;
        let at_line_end = after == bytes.len() || bytes[after] == b'\n';
        if at_line_start && at_line_end {
            return Some(abs);
        }
        search_from = abs + 3;
    }
    None
}

pub fn parse(block: &str) -> Frontmatter {
    let mut fm = Frontmatter::default();
    let lines: Vec<&str> = block.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            i += 1;
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if value.is_empty() {
            let mut items = Vec::new();
            i += 1;
            while i < lines.len() {
                let tl = lines[i].trim_start();
                if let Some(rest) = tl.strip_prefix("- ") {
                    items.push(unquote(rest));
                    i += 1;
                } else if tl.is_empty() {
                    i += 1;
                } else {
                    break;
                }
            }
            apply_list(&mut fm, key, items);
        } else {
            apply_scalar(&mut fm, key, value);
            i += 1;
        }
    }
    fm
}

fn apply_scalar(fm: &mut Frontmatter, key: &str, value: &str) {
    match key {
        "wiki" => fm.wiki = parse_bool(value),
        "title" => fm.title = Some(unquote(value)),
        "summary" => fm.summary = Some(unquote(value)),
        "tags" => fm.tags = parse_inline_list(value),
        "related" => fm.related = parse_inline_list(value),
        "aliases" => fm.aliases = parse_inline_list(value),
        _ => {}
    }
}

fn apply_list(fm: &mut Frontmatter, key: &str, items: Vec<String>) {
    match key {
        "tags" => fm.tags = items,
        "related" => fm.related = items,
        "aliases" => fm.aliases = items,
        _ => {}
    }
}

fn parse_inline_list(value: &str) -> Vec<String> {
    let t = value.trim();
    if let Some(inner) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        inner
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(unquote)
            .collect()
    } else {
        vec![unquote(t)]
    }
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2
        && ((t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')))
    {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_lowercase().as_str() {
        "true" | "yes" => Some(true),
        "false" | "no" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_frontmatter_and_body() {
        let src = "---\ntitle: Hello\n---\nbody text";
        let (fm, body) = split(src);
        let fm = fm.unwrap();
        assert_eq!(fm.title.as_deref(), Some("Hello"));
        assert_eq!(body, "body text");
    }

    #[test]
    fn parses_all_known_fields() {
        let src = r#"---
wiki: true
title: "Auth design"
summary: "認証"
tags: [auth, security]
related: [directories/src.md]
aliases:
  - auth
  - authn
---
body"#;
        let (fm, _body) = split(src);
        let fm = fm.unwrap();
        assert_eq!(fm.wiki, Some(true));
        assert_eq!(fm.title.as_deref(), Some("Auth design"));
        assert_eq!(fm.summary.as_deref(), Some("認証"));
        assert_eq!(fm.tags, vec!["auth", "security"]);
        assert_eq!(fm.related, vec!["directories/src.md"]);
        assert_eq!(fm.aliases, vec!["auth", "authn"]);
    }

    #[test]
    fn returns_none_when_no_frontmatter() {
        let (fm, body) = split("no frontmatter here");
        assert!(fm.is_none());
        assert_eq!(body, "no frontmatter here");
    }

    #[test]
    fn treats_malformed_as_no_frontmatter() {
        // 閉じが無い
        let (fm, _) = split("---\nkey: val\n\nbody");
        assert!(fm.is_none());
    }

    #[test]
    fn parses_wiki_false() {
        let (fm, _) = split("---\nwiki: false\n---\n");
        assert_eq!(fm.unwrap().wiki, Some(false));
    }
}
