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
├── index.md             # 入口。サマリ + 索引への導線
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
- 各ページの末尾には必要に応じて `## Backlinks` が付く。`## Related` は入口ページのみに付く
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

# 継続検証
scripts/verify.sh

# コミット単位の品質スコア履歴も残す
scripts/record-quality-score.sh
```
