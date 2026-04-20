pub mod go;
pub mod python;
pub mod rust;
pub mod typescript;

use std::path::{Path, PathBuf};

use crate::scan::ScannedFile;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Type,
    Constant,
}

impl SymbolKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Function => "fn",
            Self::Type => "type",
            Self::Constant => "const",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub struct LocatedSymbol {
    pub source: PathBuf,
    pub symbol: Symbol,
}

/// 走査済みファイル群からシンボルを抽出する。対応外拡張子は無視。
/// `target_root` は相対パスを絶対化してファイル読み込みするための基点。
pub fn extract_symbols(scanned: &[ScannedFile], target_root: &Path) -> Vec<LocatedSymbol> {
    let mut out = Vec::new();
    for f in scanned {
        let ext = f
            .relative_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let extractor = match ext {
            "rs" => rust::extract as fn(&str) -> Vec<Symbol>,
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => typescript::extract,
            "py" => python::extract,
            "go" => go::extract,
            _ => continue,
        };
        let abs = target_root.join(&f.relative_path);
        let Ok(content) = std::fs::read_to_string(&abs) else {
            continue;
        };
        for sym in extractor(&content) {
            out.push(LocatedSymbol {
                source: f.relative_path.clone(),
                symbol: sym,
            });
        }
    }
    out
}

/// シンボル表示用のソートキー（公開 → 非公開、種類、名前）。
pub fn sort_symbols(list: &mut [LocatedSymbol]) {
    list.sort_by(|a, b| {
        a.symbol
            .visibility
            .cmp(&b.symbol.visibility)
            .then_with(|| kind_order(a.symbol.kind).cmp(&kind_order(b.symbol.kind)))
            .then_with(|| a.symbol.name.cmp(&b.symbol.name))
            .then_with(|| a.source.cmp(&b.source))
    });
}

fn kind_order(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Type => 0,
        SymbolKind::Function => 1,
        SymbolKind::Constant => 2,
    }
}
