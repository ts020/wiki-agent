# repo-wiki agent

## プロジェクト概要

ローカルの指定ディレクトリ配下のコードベースを解析し、AI および人間が少ないコンテキストで探索・理解できる Markdown ベースの wiki ツリーを生成する CLI エージェント。

詳細な要件は `要件定義.md` を参照。

## 技術スタック

- 言語: Rust
- AI 連携: Anthropic Claude API (HTTP)

## ビルド・実行

```sh
cargo build
cargo run -- [TARGET_DIR]
```

## テスト

```sh
cargo test
```

## lint / format

```sh
cargo clippy -- -D warnings
cargo fmt --check
```

## アーキテクチャ方針

- CLI エントリポイント → ファイル走査 → 構造分析 → wiki 生成 のパイプライン構成
- 出力先デフォルト: `./repo-wiki`
- 毎回フル生成（差分更新は v1 スコープ外）
- 非破壊: 対象コードベースを一切変更しない

## コミットルール

- コミットメッセージは「何を変えたか」ではなく「なぜ変えたか（変更の意図）」を記述すること
- 変更内容の列挙ではなく、その変更が必要になった背景・目的を簡潔に伝える

## 開発指針（Linus Torvalds 方式）

- **データ構造を先に決める** — コードを書く前に型と構造体の設計を固める。正しいデータ構造があればコードは自明になる。詳細は [`docs/data-structures.md`](docs/data-structures.md) を参照
- **既存の何が駄目かを明確にしてから作る** — 「何を作らないか」を先に決める。詳細は [`docs/why.md`](docs/why.md) を参照
- **最小限で動くものをまず通す** — パイプライン全体を最短で貫通させてから各段を育てる
- **特殊ケースを排除する設計を選ぶ（good taste）** — 条件分岐やエッジケースが消える統一的な処理を志向する
- **不要な抽象化層を作らない** — 賢いコードより愚直で読めるコードを書く。「将来のため」の設計はしない
- **UIや体裁は後回し** — 正しい基盤を先に作り、その上に積む

### 作らないもの（要約）

- コード丸投げツール（Repomix等が既存） — 構造理解がない
- シンボルマップ/LSP連携（Serena等が既存） — 意味の説明がない
- SaaS/Web UI（DeepWiki等が既存） — ローカル完結しない
- C4図・UML・完全仕様書 — 探索用の索引であって網羅的文書ではない

### 中心データ構造（要約）

詳細は [`docs/data-structures.md`](docs/data-structures.md) を参照。

- **WikiNode** — wikiツリーの1ノード（= 1つの Markdown ファイル）。FR-09 に対応する title, summary, key_files, responsibilities, related, read_next を持つ
- **KeyFile** — ノードが参照する主要ファイル。パスと1行説明
- index も末端ノードも同じ WikiNode 型。特殊ケースはない
- `read_next` で深掘り、`related` で横移動。任意の深さに伸縮する

## 開発ルール

- `要件定義.md` を正とする。要件に矛盾や不明点がある場合は確認を取ること
- 未確定事項（セクション14）は実装前に相談すること
- エラー処理: 読めないファイルはスキップして継続、生成失敗ノードは簡易出力
- 除外対象: `.git`, `node_modules`, `dist`, `build`, `target`
