use anyhow::{Result, bail};
use serde::Deserialize;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Deserialize, Clone)]
pub struct NodeContent {
    pub summary: String,
    pub key_files: Vec<KeyFileContent>,
    pub responsibilities: Vec<String>,
}

#[derive(Deserialize, Clone)]
pub struct KeyFileContent {
    pub path: String,
    pub description: String,
}

/// チャンクから部分的なメタデータを抽出する
pub async fn extract_from_chunk(
    node_title: &str,
    chunk: &str,
    chunk_index: usize,
    total_chunks: usize,
) -> Result<NodeContent> {
    let prompt = format!(
        r#"テキストチャンクからメタデータを抽出せよ。読者はAIエージェントである。

これはノード「{node_title}」のチャンク {chunk_index}/{total_chunks} である。

制約:
- テキストに書かれている事実のみ記述。推測・一般論は不要
- summaryは1文。このチャンクが何を含むか
- key_filesは該当があれば。なければ空配列
- responsibilitiesはこのチャンクから読み取れるもの。3つ以内

チャンク内容:
{chunk}

JSON以外を出力するな。
{{
  "summary": "...",
  "key_files": [
    {{"path": "ファイルパス", "description": "体言止め10語以内"}}
  ],
  "responsibilities": ["...", "...", "..."]
}}"#
    );

    call_claude(&prompt).await
}

/// 複数チャンクの抽出結果を統合して最終ノードを生成する
pub async fn merge_extractions(
    node_title: &str,
    extractions: &[NodeContent],
    sibling_nodes: &str,
) -> Result<NodeContent> {
    let mut parts = String::new();
    for (i, e) in extractions.iter().enumerate() {
        parts.push_str(&format!("チャンク{}: {}\n", i + 1, e.summary));
        for r in &e.responsibilities {
            parts.push_str(&format!("  責務: {r}\n"));
        }
    }

    let prompt = format!(
        r#"チャンク抽出結果を統合してwikiノードを生成せよ。読者はAIエージェントである。

制約:
- summaryは1-2文に統合
- key_filesは全チャンクから重要なものを選定
- responsibilitiesは3つ以内に統合
- 他のノードとの重複を避ける

ノードタイトル: {node_title}

チャンク抽出結果:
{parts}

他のノード一覧:
{sibling_nodes}

JSON以外を出力するな。
{{
  "summary": "...",
  "key_files": [
    {{"path": "ファイルパス", "description": "体言止め10語以内"}}
  ],
  "responsibilities": ["...", "...", "..."]
}}"#
    );

    call_claude(&prompt).await
}

/// 単一パス生成（チャンクが1つの場合に使用 — 実質的に以前と同じ）
pub async fn generate_node_content(
    node_title: &str,
    file_contents: &str,
    sibling_nodes: &str,
) -> Result<NodeContent> {
    let prompt = format!(
        r#"ファイル群からwikiノードを生成せよ。読者はAIエージェントである。

制約:
- ファイルに書かれている事実のみ記述。推測・一般論・補足説明は不要
- summaryは1-2文。このスコープが何を含むかだけ書く
- key_filesのdescriptionは体言止め10語以内
- responsibilitiesは3つ以内
- 他のノードとの重複を避ける

ノードタイトル: {node_title}

ファイル内容:
{file_contents}

他のノード一覧:
{sibling_nodes}

JSON以外を出力するな。
{{
  "summary": "...",
  "key_files": [
    {{"path": "ファイルパス", "description": "体言止め10語以内"}}
  ],
  "responsibilities": ["...", "...", "..."]
}}"#
    );

    call_claude(&prompt).await
}

async fn call_claude(prompt: &str) -> Result<NodeContent> {
    let mut child = Command::new("claude")
        .args(["--output-format", "text", "-p", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to open stdin"))?
        .write_all(prompt.as_bytes())?;

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("claude CLI error: {stderr}");
    }

    let text = String::from_utf8(output.stdout)?;

    let json_str = match (text.find('{'), text.rfind('}')) {
        (Some(start), Some(end)) => &text[start..=end],
        _ => bail!("no JSON found in response: {text}"),
    };

    let content: NodeContent = serde_json::from_str(json_str)?;
    Ok(content)
}
