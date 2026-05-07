# md-wiki

Markdown ファイルを投げ込むと、タグ・見出し・リンクで横断できる個人用 wiki が自動で育っていく CLI ツール。

- **`.md` 専用** — ソースコードは対象外（索引化は `grep` / LSP に任せる）
- **AI / 外部 API 非依存** — 静的解析とルールベースのみ。何度でも再生成できる
- **増築型** — md を追加して再実行するだけで、index / タグ索引 / 見出し索引 / リンク索引 / バックリンクが自動更新される
- **Obsidian 互換の `[[wikilink]]`** をサポート（`![[embed]]` はプレーンリンクに縮退）
- **Agentic Search 対応** — `md_wiki` metadata、agent guide、page catalog、term index で全文一括読みなしの探索を支援

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

## Qwen3 推奨読み順

1. `index.md` で `Search Map` と `Contents Preview` を読む
2. 読み方を決める必要があれば `agent/index.md` を読む
3. 固有名・見出し語は `agent/terms/index.md` または `headings/index.md` から候補を絞る
4. 大分類から探す場合は `fragments/_index.md` から下位 `_index.md` を辿る
5. 生成ページの所在・source 範囲を確認する場合は `agent/pages/index.md` を読む
6. leaf を読む前に entry / shell / Prev / Next / Backlinks で周辺だけを広げる

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
cargo run --quiet --example agentic_search_gate -- --mode normal --work-dir target/agentic-search-gate --report target/agentic-search-gate/report.json --min-score 100

# コミット単位の品質スコア履歴も残す
scripts/record-quality-score.sh
```
