# md-wiki

Markdown ファイルを投げ込むと、タグ・見出し・リンクで横断できる個人用 wiki が自動で育っていく CLI ツール。

- **`.md` 専用** — ソースコードは対象外（索引化は `grep` / LSP に任せる）
- **AI / 外部 API 非依存** — 静的解析とルールベースのみ。何度でも再生成できる
- **増築型** — md を追加して再実行するだけで、index / タグ索引 / 見出し索引 / リンク索引 / バックリンクが自動更新される
- **Obsidian 互換の `[[wikilink]]`** をサポート（`![[embed]]` はプレーンリンクに縮退）

詳細な要件は [`docs/要件定義/index.md`](docs/要件定義/index.md) を参照。

## ビルド / 実行

```sh
cargo build

# 単一 md を wiki 化
cargo run -- path/to/note.md

# ディレクトリを再帰的に wiki 化
cargo run -- path/to/notes --recursive

# 出力先を指定
cargo run -- path/to/notes -r -o path/to/wiki
```

既定の出力先は `./md-wiki`。

## 生成されるもの

```
md-wiki/
├── index.md            # 入口。ノート数・索引への導線
├── notes/              # ノート本体（原本 + wikilink 変換 + 末尾に自動リンク欄）
├── tags/               # タグ索引（ネストタグは tags/<a>/<b>.md）
├── headings/           # 全ノートの見出し索引（h1-h2）
├── links/              # ノート間のリンク関係一覧
└── _unresolved.md      # 未解決 wikilink 一覧
```

- `notes/` 配下は原本の相対パスを維持して配置。本文は `[[wikilink]]` 変換のみで無改変
- 各ノートの末尾には `## Backlinks` と `## Related` が水平線で区切られて自動付与される
- `md-wiki/` を削除しても入力側の `.md` は一切変更されないため、何度でも再生成できる

## 取り込みルール

対象 `.md` について、フロントマターに `wiki: false` があるものだけを除外する。それ以外は取り込む。

フロントマターフィールド: `wiki` / `title` / `summary` / `tags` / `related` / `aliases`

## 除外

以下のディレクトリは走査しない：

- `.git`, `node_modules`, `dist`, `build`, `target`
- 名前が `.` で始まるディレクトリ（`.wiki` を除く）

## 開発

```sh
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```
