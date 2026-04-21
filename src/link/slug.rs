use unicode_normalization::UnicodeNormalization;

/// FR-08 準拠の見出しスラグ化。
///
/// 1. Unicode NFKC 正規化
/// 2. ASCII 英大文字は小文字化
/// 3. 空白（`\s+`）は単一の `-` に置換
/// 4. 句読点 `.,:;!?` + バッククォート・引用符・各種括弧・`@#$%^&*+=|\/~` と制御文字は除去
/// 5. 連続 `-` は 1 個に畳み、先頭末尾の `-` を除去
/// 6. 日本語・CJK・その他 Unicode 文字はそのまま残す（GitHub 風）
const PUNCT_REMOVE: &[char] = &[
    '.', ',', ':', ';', '!', '?', '`', '\'', '"', '(', ')', '[', ']', '{', '}', '<', '>', '@', '#',
    '$', '%', '^', '&', '*', '+', '=', '|', '\\', '/', '~',
];

pub fn slugify(s: &str) -> String {
    let normalized: String = s.nfkc().collect();
    let lower = normalized.to_lowercase();
    // 第 1 段階: 空白 → `-`、除去対象は落とす、それ以外はそのまま残す
    let mut stage = String::new();
    for c in lower.chars() {
        if c.is_whitespace() {
            stage.push('-');
        } else if PUNCT_REMOVE.contains(&c) || c.is_control() {
            continue;
        } else {
            stage.push(c);
        }
    }
    // 第 2 段階: 連続 `-` を 1 個に畳む
    let mut collapsed = String::new();
    let mut prev_dash = false;
    for c in stage.chars() {
        if c == '-' {
            if !prev_dash {
                collapsed.push('-');
                prev_dash = true;
            }
        } else {
            collapsed.push(c);
            prev_dash = false;
        }
    }
    // 先頭末尾の `-` を剥ぐ
    let trimmed: &str = collapsed.trim_start_matches('-').trim_end_matches('-');
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_slug() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn punctuation_removed_not_dashed() {
        // スペック: 句読点は除去。dash 化しない
        assert_eq!(slugify("A.B"), "ab");
        assert_eq!(slugify("foo!bar?"), "foobar");
    }

    #[test]
    fn whitespace_around_dash_preserves_dash() {
        // 既存の `-` はそのまま、空白は dash 化、連続 dash は畳む
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
    fn nfkc_normalizes_fullwidth_ascii() {
        // 全角英数字は ASCII 相当になる
        assert_eq!(slugify("ＡＢＣ"), "abc");
    }

    #[test]
    fn strips_leading_trailing() {
        assert_eq!(slugify(" ! hi ! "), "hi");
    }
}
