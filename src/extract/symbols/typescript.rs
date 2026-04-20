use regex::Regex;
use std::sync::OnceLock;

use super::{Symbol, SymbolKind, Visibility};

static PAT: OnceLock<Vec<(Regex, SymbolKind)>> = OnceLock::new();

fn patterns() -> &'static Vec<(Regex, SymbolKind)> {
    PAT.get_or_init(|| {
        vec![
            (
                Regex::new(
                    r"^\s*(export\s+)?(?:default\s+)?(?:async\s+)?function\s+([A-Za-z_$][A-Za-z0-9_$]*)",
                )
                .unwrap(),
                SymbolKind::Function,
            ),
            (
                Regex::new(
                    r"^\s*(export\s+)?(?:default\s+)?(?:abstract\s+)?class\s+([A-Za-z_$][A-Za-z0-9_$]*)",
                )
                .unwrap(),
                SymbolKind::Type,
            ),
            (
                Regex::new(r"^\s*(export\s+)?interface\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap(),
                SymbolKind::Type,
            ),
            (
                Regex::new(r"^\s*(export\s+)?type\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=").unwrap(),
                SymbolKind::Type,
            ),
            (
                Regex::new(r"^\s*(export\s+)?enum\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap(),
                SymbolKind::Type,
            ),
            (
                Regex::new(
                    r"^\s*(export\s+)?(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*(?::|=)",
                )
                .unwrap(),
                SymbolKind::Constant,
            ),
        ]
    })
}

pub fn extract(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    for line in content.lines() {
        if is_comment(line) {
            continue;
        }
        for (re, kind) in patterns() {
            if let Some(c) = re.captures(line) {
                let vis = if c.get(1).is_some() {
                    Visibility::Public
                } else {
                    Visibility::Private
                };
                if let Some(name) = c.get(2) {
                    out.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: *kind,
                        visibility: vis,
                    });
                }
            }
        }
    }
    out
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("*") || t.starts_with("/*")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_exports_and_internals() {
        let src = r#"
export function hello() {}
function world() {}
export class Foo {}
class Bar {}
export interface I {}
export type Alias = string;
export const MAX = 10;
let internal: number = 1;
"#;
        let syms = extract(src);
        let get = |name: &str| syms.iter().find(|s| s.name == name).cloned();
        assert_eq!(get("hello").unwrap().visibility, Visibility::Public);
        assert_eq!(get("world").unwrap().visibility, Visibility::Private);
        assert_eq!(get("Foo").unwrap().kind, SymbolKind::Type);
        assert_eq!(get("I").unwrap().kind, SymbolKind::Type);
        assert_eq!(get("Alias").unwrap().kind, SymbolKind::Type);
        assert_eq!(get("MAX").unwrap().kind, SymbolKind::Constant);
        assert_eq!(get("internal").unwrap().visibility, Visibility::Private);
    }

    #[test]
    fn ignores_comments() {
        let src = "// export function nope() {}\n/* export class X */";
        assert!(extract(src).is_empty());
    }

    #[test]
    fn detects_export_default_class_and_abstract() {
        let src = r#"
export default class Widget {}
export abstract class Shape {}
export default function make() {}
"#;
        let syms = extract(src);
        let names: Vec<_> = syms.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"Widget".to_string()));
        assert!(names.contains(&"Shape".to_string()));
        assert!(names.contains(&"make".to_string()));
    }
}
