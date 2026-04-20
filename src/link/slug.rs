/// GitHub-flavored なヘディングスラッグを生成する。
///
/// - 小文字化
/// - 英数字 / アンダースコア / Unicode 文字（日本語等）は保持
/// - それ以外の記号・空白は `-` に置換
/// - 連続する `-` は 1 個にまとめ、先頭末尾の `-` は除去
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.chars() {
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        if lower.is_alphanumeric() || lower == '_' {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_slug() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn punctuation_collapses() {
        assert_eq!(slugify("A - B / C"), "a-b-c");
    }

    #[test]
    fn underscore_preserved() {
        assert_eq!(slugify("foo_bar"), "foo_bar");
    }

    #[test]
    fn unicode_preserved() {
        assert_eq!(slugify("概要"), "概要");
        assert_eq!(slugify("見出し 1"), "見出し-1");
    }

    #[test]
    fn strips_leading_trailing() {
        assert_eq!(slugify(" ! hi ! "), "hi");
    }
}
