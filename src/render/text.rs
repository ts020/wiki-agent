use std::sync::OnceLock;

use regex::Regex;

static IMAGE_RE: OnceLock<Regex> = OnceLock::new();
static LINK_RE: OnceLock<Regex> = OnceLock::new();
static TAG_RE: OnceLock<Regex> = OnceLock::new();
static WS_RE: OnceLock<Regex> = OnceLock::new();

fn image_re() -> &'static Regex {
    IMAGE_RE.get_or_init(|| Regex::new(r"!\[([^\]\n]*)\]\([^)]+\)").unwrap())
}

fn link_re() -> &'static Regex {
    LINK_RE.get_or_init(|| Regex::new(r"\[([^\]\n]+)\]\([^)]+\)").unwrap())
}

fn tag_re() -> &'static Regex {
    TAG_RE.get_or_init(|| Regex::new(r"<[^>\n]+>").unwrap())
}

fn ws_re() -> &'static Regex {
    WS_RE.get_or_init(|| Regex::new(r"\s+").unwrap())
}

/// Markdown link labels cannot safely contain arbitrary Markdown. Headings from
/// real READMEs often include badges or inline links, so generated navigation
/// flattens labels to readable text before wrapping them in `[...]`.
pub fn link_label(raw: &str) -> String {
    let mut text = raw.to_string();
    loop {
        let next = image_re().replace_all(&text, "$1").to_string();
        if next == text {
            break;
        }
        text = next;
    }
    loop {
        let next = link_re().replace_all(&text, "$1").to_string();
        if next == text {
            break;
        }
        text = next;
    }
    text = tag_re().replace_all(&text, "").to_string();
    text = text.replace("[[", "[").replace("]]", "]");
    text = text.replace('[', "\\[").replace(']', "\\]");
    ws_re().replace_all(text.trim(), " ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_markdown_links_and_badges_for_link_labels() {
        let raw = "[React](https://react.dev/) &middot; [![GitHub license](badge.svg)](LICENSE)";
        assert_eq!(link_label(raw), "React &middot; GitHub license");
    }

    #[test]
    fn escapes_literal_brackets() {
        assert_eq!(link_label("Array [T]"), "Array \\[T\\]");
    }
}
