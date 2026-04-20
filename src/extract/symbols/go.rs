use regex::Regex;
use std::sync::OnceLock;

use super::{Symbol, SymbolKind, Visibility};

static FUNC_RE: OnceLock<Regex> = OnceLock::new();
static TYPE_RE: OnceLock<Regex> = OnceLock::new();
static CONST_RE: OnceLock<Regex> = OnceLock::new();

fn func_re() -> &'static Regex {
    FUNC_RE.get_or_init(|| {
        // func (receiver) Name(args) ret { ... }
        Regex::new(r"^func\s+(?:\([^)]*\)\s+)?([A-Za-z_][A-Za-z0-9_]*)").unwrap()
    })
}

fn type_re() -> &'static Regex {
    TYPE_RE.get_or_init(|| Regex::new(r"^type\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap())
}

fn const_re() -> &'static Regex {
    CONST_RE.get_or_init(|| Regex::new(r"^(?:const|var)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap())
}

pub fn extract(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    for line in content.lines() {
        if is_comment(line) {
            continue;
        }
        if let Some(c) = func_re().captures(line)
            && let Some(name) = c.get(1)
        {
            push(&mut out, name.as_str(), SymbolKind::Function);
        }
        if let Some(c) = type_re().captures(line)
            && let Some(name) = c.get(1)
        {
            push(&mut out, name.as_str(), SymbolKind::Type);
        }
        if let Some(c) = const_re().captures(line)
            && let Some(name) = c.get(1)
        {
            push(&mut out, name.as_str(), SymbolKind::Constant);
        }
    }
    out
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with("*")
}

fn push(out: &mut Vec<Symbol>, name: &str, kind: SymbolKind) {
    let first = name.chars().next().unwrap_or('_');
    let vis = if first.is_uppercase() {
        Visibility::Public
    } else {
        Visibility::Private
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
    fn detects_funcs_types_consts_and_visibility() {
        let src = r#"
package main

func Hello() {}
func helperInternal() {}
func (r *Receiver) Method() {}

type Foo struct{}
type bar interface{}

const MaxN = 10
var internalVar = 1
"#;
        let syms = extract(src);
        let get = |n: &str| syms.iter().find(|s| s.name == n).cloned();
        assert_eq!(get("Hello").unwrap().visibility, Visibility::Public);
        assert_eq!(
            get("helperInternal").unwrap().visibility,
            Visibility::Private
        );
        assert_eq!(get("Method").unwrap().visibility, Visibility::Public);
        assert_eq!(get("Foo").unwrap().kind, SymbolKind::Type);
        assert_eq!(get("bar").unwrap().visibility, Visibility::Private);
        assert_eq!(get("MaxN").unwrap().kind, SymbolKind::Constant);
        assert_eq!(get("internalVar").unwrap().visibility, Visibility::Private);
    }

    #[test]
    fn ignores_comments() {
        let src = "// func Hidden() {}\n/* type X struct{} */";
        assert!(extract(src).is_empty());
    }
}
