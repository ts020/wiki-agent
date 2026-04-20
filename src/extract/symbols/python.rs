use regex::Regex;
use std::sync::OnceLock;

use super::{Symbol, SymbolKind, Visibility};

static DEF_RE: OnceLock<Regex> = OnceLock::new();
static CLASS_RE: OnceLock<Regex> = OnceLock::new();

fn def_re() -> &'static Regex {
    DEF_RE.get_or_init(|| Regex::new(r"^(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap())
}

fn class_re() -> &'static Regex {
    CLASS_RE.get_or_init(|| Regex::new(r"^class\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap())
}

/// モジュール直下（インデント 0）の def / class のみ抽出する。
/// 名前が `_` 始まりの場合は非公開扱い。
pub fn extract(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    for line in content.lines() {
        if is_comment(line) {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        if let Some(c) = def_re().captures(line)
            && let Some(name) = c.get(1)
        {
            push(&mut out, name.as_str(), SymbolKind::Function);
        }
        if let Some(c) = class_re().captures(line)
            && let Some(name) = c.get(1)
        {
            push(&mut out, name.as_str(), SymbolKind::Type);
        }
    }
    out
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

fn push(out: &mut Vec<Symbol>, name: &str, kind: SymbolKind) {
    let vis = if name.starts_with('_') {
        Visibility::Private
    } else {
        Visibility::Public
    };
    out.push(Symbol {
        name: name.to_string(),
        kind,
        visibility: vis,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_module_level_def_and_class() {
        let src = r#"
def foo():
    pass

async def bar():
    pass

def _private():
    pass

class Public:
    def method(self):
        pass

class _Private:
    pass
"#;
        let syms = extract(src);
        let get = |n: &str| syms.iter().find(|s| s.name == n).cloned();
        assert_eq!(get("foo").unwrap().visibility, Visibility::Public);
        assert_eq!(get("bar").unwrap().visibility, Visibility::Public);
        assert_eq!(get("_private").unwrap().visibility, Visibility::Private);
        assert_eq!(get("Public").unwrap().kind, SymbolKind::Type);
        assert_eq!(get("_Private").unwrap().visibility, Visibility::Private);
        assert!(get("method").is_none());
    }

    #[test]
    fn ignores_comments() {
        let src = "# def hidden():\n#     pass";
        assert!(extract(src).is_empty());
    }

    #[test]
    fn decorator_on_previous_line_does_not_prevent_detection() {
        // デコレータ行はパターンにマッチしないのでスキップされ、次行が判定対象になる
        let src = r#"
@dataclass
class Config:
    pass

@staticmethod
def helper():
    pass
"#;
        let syms = extract(src);
        let names: Vec<_> = syms.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"Config".to_string()));
        assert!(names.contains(&"helper".to_string()));
    }
}
