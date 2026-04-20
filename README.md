# repo-wiki agent

ローカルのコードベースと手書き Markdown ノートを解析し、AI および人間が少ないコンテキストで探索・理解できる Markdown ベースの wiki ツリーを生成する CLI。

- AI / 外部 API 非依存（v1 はすべて静的解析 + ルールベース）
- 毎回フル生成。md を追加して再実行すれば index/タグ/バックリンクが自動で増築される
- Obsidian 互換の `[[wikilink]]` をサポート

詳細な要件は [`docs/要件定義/index.md`](docs/要件定義/index.md) を参照。

## ビルド / 実行

```sh
cargo build
cargo run -- [TARGET_DIR]        # 未指定時はカレント、出力は ./repo-wiki
cargo run -- path/to/repo --output path/to/wiki
```

## 生成されるもの

```
repo-wiki/
├── index.md              # プロジェクト概要と全ノードへの導線
├── overview/             # 技術スタック / エントリポイント / テスト構造
├── directories/          # コード由来ノード（ディレクトリ単位）
├── notes/                # 手書き md ノート由来
├── tags/                 # タグ索引（ネストタグ対応）
├── development/          # ビルド/テストコマンド案内
└── _unresolved.md        # 未解決 wikilink 一覧
```

各ノードは Summary / Key files / Structure / Related / Read next / Backlinks を含む。

## 手書き md の取り込みルール

以下の優先順で判定する（先に該当したルールで確定）：

1. フロントマターに `wiki: false` → 除外
2. フロントマターに `wiki: true` → 取り込み
3. プロジェクトルート直下の `README.md`
4. `docs/` / `notes/` / `.wiki/` 配下（再帰的）
5. それ以外は無視

フロントマターフィールド: `wiki` / `title` / `summary` / `tags` / `related` / `aliases`

## v1 シンボル抽出対応言語

Rust / TypeScript・JavaScript / Python / Go（正規表現ベース、FP/FN あり）

## 開発

```sh
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```
