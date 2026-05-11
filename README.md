# md-wiki

[![Release](https://img.shields.io/github/v/release/ts020/wiki-agent?display_name=tag&sort=semver)](https://github.com/ts020/wiki-agent/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/ts020/wiki-agent/actions/workflows/verify.yml/badge.svg)](https://github.com/ts020/wiki-agent/actions/workflows/verify.yml)

Markdown ファイルを、タグ・見出し・リンクで横断できるオフライン wiki へ変換する CLI ツールです。入力はローカルの `.md` だけで、生成物も Markdown のまま残ります。

- **`.md` 専用** — ソースコードは対象外（索引化は `grep` / LSP に任せる）
- **AI / 外部 API 非依存** — 静的解析とルールベースのみ。何度でも再生成できる
- **増築型** — md を追加して再実行するだけで、index / タグ索引 / 見出し索引 / リンク索引 / バックリンクが自動更新される
- **Obsidian 互換の `[[wikilink]]`** をサポート（`![[embed]]` はプレーンリンクに縮退）
- **Agentic Search 対応** — `md_wiki` metadata、agent guide、page catalog、term index で全文一括読みなしの探索を支援

詳細な要件は [`docs/要件定義/index.md`](docs/要件定義/index.md) を参照。

## インストール

### curl ワンライナー（macOS / Linux）

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ts020/wiki-agent/releases/latest/download/md-wiki-cli-installer.sh | sh
```

### PowerShell ワンライナー（Windows）

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/ts020/wiki-agent/releases/latest/download/md-wiki-cli-installer.ps1 | iex"
```

どちらも [cargo-dist](https://github.com/axodotdev/cargo-dist) 生成の installer で、GitHub Release から対象 OS のアーカイブを取得し、checksum 検証のうえ既定では `$CARGO_HOME/bin/md-wiki`（未設定なら `~/.cargo/bin/md-wiki`）にインストールします。

### バージョンを固定する

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ts020/wiki-agent/releases/download/v0.1.3/md-wiki-cli-installer.sh | sh
```

### インストール先を変える

`MD_WIKI_CLI_INSTALL_DIR` を指定すると、その配下の `bin/` に binary が置かれます（hierarchical layout）。たとえば `MD_WIKI_CLI_INSTALL_DIR="$HOME/opt/md-wiki"` を渡すと、実体は `$HOME/opt/md-wiki/bin/md-wiki` に展開されます。

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ts020/wiki-agent/releases/latest/download/md-wiki-cli-installer.sh \
  | MD_WIKI_CLI_INSTALL_DIR="$HOME/opt/md-wiki" MD_WIKI_CLI_NO_MODIFY_PATH=1 sh
```

`MD_WIKI_CLI_NO_MODIFY_PATH=1` を設定すると shell rc ファイルへの自動 PATH 追記を抑止できます。

crates.io の package 名は `md-wiki-cli` ですが、binary name is still `md-wiki` です。

### 対応プラットフォーム

| OS | アーキテクチャ | アセット |
|---|---|---|
| Linux  | x86_64  | `md-wiki-cli-x86_64-unknown-linux-gnu.tar.xz` |
| Linux  | aarch64 | `md-wiki-cli-aarch64-unknown-linux-gnu.tar.xz` |
| macOS  | x86_64  | `md-wiki-cli-x86_64-apple-darwin.tar.xz` |
| macOS  | aarch64 (Apple Silicon) | `md-wiki-cli-aarch64-apple-darwin.tar.xz` |
| Windows | x86_64 | `md-wiki-cli-x86_64-pc-windows-msvc.zip` |

### 手動インストール

[Releases ページ](https://github.com/ts020/wiki-agent/releases) から対象 OS の archive を取得し、`md-wiki` (Windows は `md-wiki.exe`) を `PATH` の通った場所に置きます。

```sh
tar -xJf md-wiki-cli-x86_64-unknown-linux-gnu.tar.xz
install -m 0755 md-wiki-cli-x86_64-unknown-linux-gnu/md-wiki ~/.local/bin/md-wiki
```

checksum 検証:

```sh
curl -fsSLO https://github.com/ts020/wiki-agent/releases/latest/download/sha256.sum
sha256sum -c sha256.sum --ignore-missing
```

### ソースからビルド

```sh
# clone 済みの作業ツリーから
cargo install --path .

# 開発中に直接実行
cargo run -- path/to/notes -r -o path/to/wiki
```

### 動作確認

```sh
md-wiki --version
md-wiki --help
```

### アップグレード / アンインストール

```sh
# 最新へ更新（既存と同じ場所にインストールされる）
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ts020/wiki-agent/releases/latest/download/md-wiki-cli-installer.sh | sh

# 削除（既定インストール先）
rm "$HOME/.cargo/bin/md-wiki"
```

## 最小例

入力ディレクトリを作ります。

```sh
mkdir -p notes
cat > notes/start.md <<'MD'
---
title: Start
tags: [project/demo]
---

# Start

[[Next]] へ進む。

## Overview

最初のメモ。
MD

cat > notes/next.md <<'MD'
# Next

Start からリンクされるメモ。
MD
```

wiki を生成します。

```sh
md-wiki notes --recursive --out md-wiki
```

主な出力を確認します。

```sh
ls md-wiki
sed -n '1,80p' md-wiki/index.md
sed -n '1,80p' md-wiki/fragments/start/index.md
sed -n '1,80p' md-wiki/tags/project.md
```

開発中は同じ操作を `cargo run -- notes -r -o md-wiki` でも実行できます。既定の出力先は `./md-wiki` です。

## 生成されるもの

```
md-wiki/
├── index.md             # 入口。サマリ + 索引への導線
├── agent/               # tool-use agent 向け guide / catalog / term index
├── fragments/           # ノート本体（入口ページ + h2/h3 断片ページ）
│   ├── _index.md        # 階層サマリ
│   └── <rel>/
│       ├── index.md     # ノート入口ページ
│       └── <h2>.md      # h2 断片ページ
├── tags/                # タグ索引（ネストタグは tags/<a>/<b>.md）
├── headings/            # 全ノートの見出し索引（h1-h2）
├── links/               # ノート間のリンク関係一覧
└── _unresolved.md       # 未解決 wikilink 一覧
```

- `fragments/` 配下は原本の相対パスを維持して配置。本文は `[[wikilink]]` 変換のみで無改変
- h2 を持つノートは入口ページと断片ページに分割される。長い h2 は条件により h3 子断片へ再分割される
- すべての生成 `.md` は YAML frontmatter の `md_wiki` metadata を持つ
- 1 MiB 超の UTF-8 `.md` は skip せず large path で 40,000 文字以内の leaf 群へ分割される
- 各ページの末尾には必要に応じて `## Backlinks` が付く。`## Related` は入口ページのみに付く
- `md-wiki/` を削除しても入力側の `.md` は一切変更されないため、何度でも再生成できる

## 取り込みルール

対象 `.md` について、フロントマターに `wiki: false` があるものだけを除外します。それ以外は取り込みます。

```yaml
---
wiki: false
---
```

断片化せず 1 ページにまとめたいノートでは、`fragment: false` を指定します。

```yaml
---
fragment: false
---
```

フロントマターで扱う主なフィールド:

| フィールド | 用途 |
|---|---|
| `title` | ノート表示名。未指定時は h1、なければファイル名 |
| `summary` | 索引や関連ノートで使う短い説明 |
| `tags` | `tags/` 索引に出すタグ。`a/b` のネスト表記に対応 |
| `related` | 入口ページに表示する関連ノート |
| `aliases` | `[[wikilink]]` 解決時の別名 |
| `fragment` | `false` の場合は h2/h3 分割せず入口ページに全文を置く |
| `wiki` | `false` の場合だけ取り込み対象から外す |

## 対応する Markdown 構文

- `[[Foo]]` は対象ノートの入口ページへ変換
- `[[Foo#見出し]]` は対応する h2 断片または入口ページ内アンカーへ変換
- `[[Foo|ラベル]]` はラベル付きリンクとして変換
- `![[Foo]]` は v1 では埋め込みではなく通常のリンクとして扱う
- 未解決の `[[...]]` は本文に `(未解決)` を残し、`_unresolved.md` に集約
- フロントマター `tags`、h1/h2、ノート間リンク、バックリンクを索引化

本文中の `#tag`、HTML 出力、全文検索 index、watch mode、差分更新は v0.1.0 の対象外です。

## 安全性と制限

- 入力側の `.md` は変更しない。生成は指定された出力先だけに行う
- 出力先が既存ディレクトリの場合、本ツール由来と推定できない内容を壊さないよう保護する
- `.git`, `node_modules`, `dist`, `build`, `target` と、`.wiki` 以外の隠しディレクトリは走査しない
- NULL バイトを含むファイル、UTF-8 として解釈できないファイル、読み取り不能ファイルは警告してスキップする
- 外部 API、AI、embedding、ネットワーク接続は使わない
- 1 MiB 超の UTF-8 `.md` は large path へ分類し、agentic search 向けの小さなページ群へ分割する。200 MiB 超は管理上限を超える可能性があるため、リリース前検証で個別確認する

md-wiki は Markdown を HTML サイトへ変換する静的サイトジェネレータではありません。生成物も Markdown のままにして、ローカル閲覧、grep、エディタ、tool-use agent の探索に使いやすくすることを優先しています。

## Qwen3 推奨読み順

1. `index.md` で `Search Map` と `Contents Preview` を読む
2. 読み方を決める必要があれば `agent/index.md` を読む
3. 固有名・見出し語は `agent/terms/index.md` または `headings/index.md` から候補を絞る
4. 大分類から探す場合は `fragments/_index.md` から下位 `_index.md` を辿る
5. 生成ページの所在・source 範囲を確認する場合は `agent/pages/index.md` を読む
6. leaf を読む前に entry / shell / Prev / Next / Backlinks で周辺だけを広げる

## 開発

```sh
cargo test
cargo clippy -- -D warnings
cargo fmt --check

# 継続検証
scripts/verify.sh
cargo run --quiet --example agentic_search_gate -- --mode normal --work-dir target/agentic-search-gate --report target/agentic-search-gate/report.json --min-score 100

# コミット単位の品質スコア履歴も残す
scripts/record-quality-score.sh
```

公開前には以下も確認します。

```sh
cargo test --locked
cargo package --locked
cargo package --no-verify --list
```

## コントリビュート

仕様判断は [`docs/要件定義/`](docs/要件定義/) を正とします。変更前に [CONTRIBUTING.md](CONTRIBUTING.md) を読み、生成仕様を変える場合は要件定義とテストを同じ変更に含めてください。

リリース手順は [`docs/RELEASING.md`](docs/RELEASING.md) を参照してください。

## ライセンス

MIT License. 詳細は [LICENSE](LICENSE) を参照してください。
