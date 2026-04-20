use regex::Regex;
use std::sync::OnceLock;

use super::{Symbol, SymbolKind, Visibility};

static FN_RE: OnceLock<Regex> = OnceLock::new();
static STRUCT_RE: OnceLock<Regex> = OnceLock::new();
static ENUM_RE: OnceLock<Regex> = OnceLock::new();
static TRAIT_RE: OnceLock<Regex> = OnceLock::new();
static TYPE_RE: OnceLock<Regex> = OnceLock::new();
static CONST_RE: OnceLock<Regex> = OnceLock::new();

fn fn_re() -> &'static Regex {
    FN_RE.get_or_init(|| {
        Regex::new(r"^\s*(pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()
    })
}

fn struct_re() -> &'static Regex {
    STRUCT_RE.get_or_init(|| {
        Regex::new(r"^\s*(pub(?:\([^)]*\))?\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()
    })
}

fn enum_re() -> &'static Regex {
    ENUM_RE.get_or_init(|| {
        Regex::new(r"^\s*(pub(?:\([^)]*\))?\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()
    })
}

fn trait_re() -> &'static Regex {
    TRAIT_RE.get_or_init(|| {
        Regex::new(r"^\s*(pub(?:\([^)]*\))?\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()
    })
}

fn type_re() -> &'static Regex {
    TYPE_RE.get_or_init(|| {
        Regex::new(r"^\s*(pub(?:\([^)]*\))?\s+)?type\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap()
    })
}

fn const_re() -> &'static Regex {
    CONST_RE.get_or_init(|| {
        Regex::new(r"^\s*(pub(?:\([^)]*\))?\s+)?(?:const|static)\s+([A-Z_][A-Z0-9_]*)\s*:").unwrap()
    })
}

pub fn extract(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    for line in content.lines() {
        if is_comment(line) {
            continue;
        }
        push(&mut out, line, fn_re(), SymbolKind::Function);
        push(&mut out, line, struct_re(), SymbolKind::Type);
        push(&mut out, line, enum_re(), SymbolKind::Type);
        push(&mut out, line, trait_re(), SymbolKind::Type);
        push(&mut out, line, type_re(), SymbolKind::Type);
        push(&mut out, line, const_re(), SymbolKind::Constant);
    }
    out
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("*") || t.starts_with("/*")
}

fn push(out: &mut Vec<Symbol>, line: &str, re: &Regex, kind: SymbolKind) {
    if let Some(c) = re.captures(line) {
        let vis = if c.get(1).is_some() {
            Visibility::Public
        } else {
            Visibility::Private
        };
        if let Some(name) = c.get(2) {
            out.push(Symbol {
                name: name.as_str().to_string(),
                kind,
                visibility: vis,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_public_and_private_fns() {
        let src = "pub fn a() {}\nfn b() {}\nasync fn c() {}\npub async fn d() {}";
        let syms = extract(src);
        assert_eq!(syms.len(), 4);
        assert_eq!(syms[0].name, "a");
        assert_eq!(syms[0].visibility, Visibility::Public);
        assert_eq!(syms[1].name, "b");
        assert_eq!(syms[1].visibility, Visibility::Private);
    }

    #[test]
    fn detects_types() {
        let src = "pub struct S;\nenum E {}\npub(crate) trait T {}\ntype Alias = u8;";
        let syms = extract(src);
        let names: Vec<_> = syms.iter().map(|s| (s.name.clone(), s.kind)).collect();
        assert!(names.contains(&("S".to_string(), SymbolKind::Type)));
        assert!(names.contains(&("E".to_string(), SymbolKind::Type)));
        assert!(names.contains(&("T".to_string(), SymbolKind::Type)));
        assert!(names.contains(&("Alias".to_string(), SymbolKind::Type)));
    }

    #[test]
    fn ignores_comments() {
        let src = "// pub fn hidden() {}\n/* pub struct X */";
        assert!(extract(src).is_empty());
    }

    #[test]
    fn detects_consts() {
        let src = "pub const MAX: u32 = 1;\nstatic FOO: u32 = 2;";
        let syms = extract(src);
        assert_eq!(syms.len(), 2);
        assert_eq!(syms[0].kind, SymbolKind::Constant);
    }

    #[test]
    fn detects_async_unsafe_and_const_variants() {
        // 公式に許される順序: const? async? unsafe?
        let src = r#"
pub async unsafe fn a() {}
pub const fn b() {}
const unsafe fn c() {}
async unsafe fn d() {}
"#;
        let syms = extract(src);
        let names: Vec<_> = syms.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
        assert!(names.contains(&"c".to_string()));
        assert!(names.contains(&"d".to_string()));
    }
}
