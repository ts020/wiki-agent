use anyhow::{Result, bail};
use serde::Deserialize;
use std::process::Command;

#[derive(Deserialize)]
pub struct NodeContent {
    pub summary: String,
    pub key_files: Vec<KeyFileContent>,
    pub responsibilities: Vec<String>,
}

#[derive(Deserialize)]
pub struct KeyFileContent {
    pub path: String,
    pub description: String,
}

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

    let output = Command::new("claude")
        .args(["-p", &prompt, "--output-format", "text"])
        .output()?;

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
