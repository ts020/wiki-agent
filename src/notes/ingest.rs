use std::path::{Path, PathBuf};

use super::frontmatter::{self, Frontmatter};
use super::headings::{self, Heading};
use crate::scan::ScannedFile;

#[derive(Debug, Clone)]
pub struct NoteData {
    pub source_file: PathBuf,
    pub frontmatter: Frontmatter,
    pub headings: Vec<Heading>,
    pub first_paragraph: Option<String>,
    pub body: String,
}

/// 走査済みファイルから取り込み対象のノートを収集する（§7.2）。
/// `.md` 以外はスキップ。フロントマターに `wiki: false` が書かれているものだけ除外し、
/// それ以外はすべて取り込む。
pub fn ingest_notes(scanned: &[ScannedFile], target_root: &Path) -> Vec<NoteData> {
    let mut out = Vec::new();
    for f in scanned {
        if f.relative_path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let abs = target_root.join(&f.relative_path);
        let content = match std::fs::read_to_string(&abs) {
            Ok(c) => c,
            Err(err) => {
                tracing::warn!(
                    path = %f.relative_path.display(),
                    error = %err,
                    "failed to read markdown"
                );
                continue;
            }
        };
        let (fm_opt, body) = frontmatter::split(&content);
        let fm = fm_opt.unwrap_or_default();

        if !should_ingest(&fm) {
            continue;
        }

        out.push(NoteData {
            source_file: f.relative_path.clone(),
            headings: headings::extract(&body),
            first_paragraph: first_paragraph(&body),
            frontmatter: fm,
            body,
        });
    }
    out.sort_by(|a, b| a.source_file.cmp(&b.source_file));
    out
}

/// §7.2: `wiki: false` だけを除外し、それ以外は取り込む。
pub fn should_ingest(fm: &Frontmatter) -> bool {
    fm.wiki != Some(false)
}

/// フロントマター除去後の本文から「最初の段落」を抽出する。
/// 見出し・空行・コードフェンスはスキップ。
fn first_paragraph(body: &str) -> Option<String> {
    let mut in_fence = false;
    let mut para: Vec<&str> = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if t.is_empty() {
            if !para.is_empty() {
                break;
            }
            continue;
        }
        if t.starts_with('#') {
            continue;
        }
        para.push(t);
    }
    if para.is_empty() {
        None
    } else {
        Some(para.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiki_false_excludes() {
        let fm = Frontmatter {
            wiki: Some(false),
            ..Default::default()
        };
        assert!(!should_ingest(&fm));
    }

    #[test]
    fn wiki_true_includes() {
        let fm = Frontmatter {
            wiki: Some(true),
            ..Default::default()
        };
        assert!(should_ingest(&fm));
    }

    #[test]
    fn no_wiki_flag_includes() {
        assert!(should_ingest(&Frontmatter::default()));
    }

    #[test]
    fn first_paragraph_skips_heading_and_fence() {
        let body =
            "# Title\n\n```\ncode\n```\n\nThis is the first para.\nstill first.\n\nsecond para.";
        assert_eq!(
            first_paragraph(body).as_deref(),
            Some("This is the first para. still first.")
        );
    }
}
