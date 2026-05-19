# md-wiki Retrieval Compiler Requirements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reframe `docs/要件定義/` around the approved retrieval compiler product definition without changing Rust behavior yet.

**Architecture:** Treat the approved design doc as the north star, then update the requirements in layers: product definition first, current v1 compile baseline second, schema/context retrieval requirements third, and acceptance criteria last. Keep existing v1 behavior as the baseline compile layer while moving large Markdown and agentic search material into explicit future/implementation tracks.

**Tech Stack:** Markdown requirements docs, existing md-wiki Rust project conventions, `rg`, `git diff --check`, optional `cargo test` only if code or tests are touched.

---

## File Structure

The plan is documentation-only. Do not change Rust source code in this pass.

- Modify: `docs/要件定義/index.md`  
  Update title, table of contents, and revision history so the document is framed as a retrieval compiler spec rather than a human-first wiki spec.

- Modify: `docs/要件定義/01-概要.md`  
  Replace the current one-sentence definition and purpose with the approved retrieval compiler definition.

- Modify: `docs/要件定義/02-スコープ.md`  
  Separate in-scope compile layer, retrieval layer, schema pack support, and non-goals.

- Modify: `docs/要件定義/03-想定利用者.md`  
  Make tool-using agents the primary user and humans the operator/reviewer.

- Modify: `docs/要件定義/04-用語定義.md`  
  Add core terms: corpus, schema pack, field, context recipe, entity, context pack, internal catalog.

- Modify: `docs/要件定義/05-ユースケース.md`  
  Replace stale wiki-first examples with retrieval compiler use cases, led by game narrative `twist-payoff`.

- Modify: `docs/要件定義/07-入力.md`  
  Add `--schema` input and schema YAML validation requirements.

- Modify: `docs/要件定義/08-機能要件.md`  
  Add field extraction, schema field catalogs, `.md-wiki/catalog.json`, and `md-wiki context` behavior as first-class functional requirements.

- Modify: `docs/要件定義/09-非機能要件.md`  
  Reframe context efficiency around small-context agents and deterministic retrieval.

- Modify: `docs/要件定義/10-出力要件.md`  
  Add `agent/fields/` and `.md-wiki/catalog.json`; fix root `index.md` contradiction.

- Modify: `docs/要件定義/11-制約.md`  
  Explicitly prohibit LLM calls, embedding, semantic inference, network dependencies, and arbitrary code execution from schema packs.

- Modify: `docs/要件定義/12-エラー処理.md`  
  Add schema validation errors, missing/stale catalog errors, and missing required evidence behavior.

- Modify: `docs/要件定義/13-受け入れ基準.md`  
  Keep AC-01 to AC-23 as compile baseline where still valid, fix contradictions, and add retrieval compiler acceptance criteria.

- Modify: `docs/要件定義/14-将来拡張.md`  
  Move scoring DSL, JSON context output, semantic search, embeddings, and domain-specific schema packs beyond the initial game example into future scope.

- Modify: `docs/要件定義/15-確定済み仕様.md`  
  Update decisions to reflect retrieval compiler direction and mark v1 wiki behavior as baseline compile behavior.

- Modify: `docs/要件定義/16-設計判断.md`  
  Add rationale for compiler + retrieval CLI, external YAML schema packs, internal JSON catalog, and no-inference boundary.

- Modify: `docs/要件定義/17-継続検証と品質スコア.md`  
  Add validation matrix entries for schema loading, catalog generation, context pack formatting, missing evidence, budget, and determinism.

- Modify: `docs/要件定義/18-巨大Markdown対応完了条件.md` through `docs/要件定義/21-agentic-search実装作業計画.md`  
  Do not rewrite deeply in this pass. Add short status notes that these are implementation tracks under the retrieval compiler product definition and may need follow-up refactoring.

- Reference only: `docs/superpowers/specs/2026-05-19-md-wiki-retrieval-compiler-design.md`  
  This is the approved design source. Do not edit unless the implementation reveals a contradiction in the design.

## Task 1: Reframe Product Definition and Scope

**Files:**
- Modify: `docs/要件定義/index.md`
- Modify: `docs/要件定義/01-概要.md`
- Modify: `docs/要件定義/02-スコープ.md`
- Modify: `docs/要件定義/03-想定利用者.md`

- [ ] **Step 1: Review the approved design source**

Run:

```sh
rtk sed -n '1,120p' docs/superpowers/specs/2026-05-19-md-wiki-retrieval-compiler-design.md
```

Expected: The output includes `プロダクト定義`, `主要ユースケース`, and the two-layer Compile/Retrieval description.

- [ ] **Step 2: Update `index.md` title and revision history**

Edit `docs/要件定義/index.md`.

Change the first heading to:

```md
# md-wiki Retrieval Compiler 要件定義書
```

Change the introduction paragraph to:

```md
本書は `md-wiki` の要件定義である。`md-wiki` は、ローカル Markdown corpus を小さいコンテキストの agent が探索・引用・再利用できる形へ変換する、オフラインの retrieval compiler である。実装判断は本書を正とする。
```

Add this item at the top of `## 改訂履歴`:

```md
- vNext-retrieval-compiler (2026-05-19): `md-wiki` の主目的を人間向け wiki 生成から、小さいコンテキストの agent 向け retrieval compiler へ再定義。静的 compile artifacts と `context` retrieval CLI、外部 YAML schema pack、Markdown context pack、内部 catalog を中核仕様として追加
```

- [ ] **Step 3: Rewrite `01-概要.md` around retrieval compiler**

Edit `docs/要件定義/01-概要.md`.

Set `## 1.2 一文定義` to:

```md
ローカル Markdown corpus を、小さいコンテキストの agent が探索・引用・再利用できる決定的な artifacts と Markdown context pack へ変換する CLI ツール。
```

Replace `## 1.4 目的` with:

```md
## 1.4 目的
- ローカル Markdown corpus を、agent が少ないコンテキストで段階的に探索できる構造へ変換する
- `init` / `add` で決定的な compile artifacts を生成し、`context` CLI で task 向け context pack を返す
- frontmatter、見出し、wikilink、タグ、schema pack YAML で明示された field を収集し、根拠 path / source range と共に提示する
- LLM 呼び出し、embedding、未明示の意味推論、最終文章生成を行わず、外部 agent が推論するための根拠を準備する
- 成果物は人間にも確認できる Markdown を基本とし、CLI retrieval 用の内部 catalog は `.md-wiki/` 配下の機械生成 JSON として保持する
```

Replace `## 1.5 生成物の性質` with:

```md
## 1.5 生成物の性質
生成物は「自然文の要約」ではなく「探索・引用・context pack 化のための構造索引」である。原本の本文は変更せず、生成側で source path、source range、field、entity、link graph を追跡する。

人間や agent が直接読む成果物は Markdown とする。`md-wiki context` が安定して retrieval するための内部データは `.md-wiki/catalog.json` に保存する。
```

- [ ] **Step 4: Rewrite `02-スコープ.md`**

Edit `docs/要件定義/02-スコープ.md`.

Replace `## 2.1 In Scope` with:

```md
## 2.1 In Scope
- CLI 実行
- 単一 `.md` ファイルの処理
- ディレクトリ入力の再帰走査（既定）と `--no-recursive` による直下限定走査
- `init` / `add` による compile artifacts 生成
- フロントマター（YAML）、見出し、wikilink、タグ、通常 Markdown link の構造抽出
- ノートの自動断片化と source range 追跡
- 外部 YAML schema pack の読み込みと検証
- schema pack による field 抽出と field catalog 生成
- `.md-wiki/catalog.json` による内部 retrieval catalog 生成
- `md-wiki context` による Markdown context pack 生成
- context recipe に基づく section 構成、required field 報告、Source Trail 出力
- タグ索引、見出し索引、リンク索引、バックリンク、未解決リンク一覧
- 小さいコンテキストの agent が file/list/search/read または `context` CLI で探索できる Markdown artifacts
```

Replace `## 2.2 Out of Scope` with:

```md
## 2.2 Out of Scope
- ソースコード（`.rs` `.ts` `.py` 等）の解析・シンボル抽出
- LLM 呼び出し
- AI / LLM による自然文要約生成
- embedding または vector database
- 自然言語の意味的類似検索
- 原文からの未明示の意味推論（例: 自動伏線判定、感情推定、関係性推定）
- 最終文章生成
- schema pack 内の任意コード実行
- ファイル監視による自動再実行（watch モード）
- IDE 連携
- 実行時解析
- UML / 図生成
- `.md` 以外の形式の取り込み（`.org`, `.rst`, `.txt` 等）
```

- [ ] **Step 5: Update `03-想定利用者.md`**

Edit `docs/要件定義/03-想定利用者.md`.

Replace the user categories with:

```md
## 主利用者
- ローカルファイル操作ツールと CLI 実行能力を持つ agent
- 小さいコンテキストで Markdown corpus を探索し、必要な根拠だけを LLM に渡したい agent

## 副次利用者
- corpus と schema pack を整備する人間の作者・開発者・業務担当者
- 生成された Markdown artifacts や context pack をレビューする人間

## 想定しない利用者
- `md-wiki` 自体に文章生成や推論を任せたい利用者
- embedding search や vector database による類似検索を求める利用者
- ソースコードの索引を求める開発者（`grep` / LSP / 既存ツールで代替可能）
- 大規模な共同編集ワークフローを必要とするチーム
```

- [ ] **Step 6: Verify Task 1 diff**

Run:

```sh
rtk git diff -- docs/要件定義/index.md docs/要件定義/01-概要.md docs/要件定義/02-スコープ.md docs/要件定義/03-想定利用者.md
```

Expected: The diff consistently frames `md-wiki` as a retrieval compiler and does not introduce code changes.

- [ ] **Step 7: Commit Task 1**

Run:

```sh
rtk git add docs/要件定義/index.md docs/要件定義/01-概要.md docs/要件定義/02-スコープ.md docs/要件定義/03-想定利用者.md
rtk git commit -m "Reframe md-wiki around agent retrieval"
```

Expected: A commit is created.

## Task 2: Add Core Retrieval Terms and Use Cases

**Files:**
- Modify: `docs/要件定義/04-用語定義.md`
- Modify: `docs/要件定義/05-ユースケース.md`

- [ ] **Step 1: Update terms**

Edit `docs/要件定義/04-用語定義.md`.

Keep existing terms that still apply. Add these terms after `wiki ツリー`:

```md
- **corpus**: 入力となる Markdown ファイル群。単一 `.md` またはディレクトリ配下の `.md` 群を指す
- **compile artifacts**: `init` / `add` が生成する Markdown 成果物と `.md-wiki/` 配下の機械生成管理ファイル
- **schema pack**: field 抽出と context recipe を定義する外部 YAML ファイル
- **field**: schema pack が定義する収集単位。例: `canon`, `setup`, `expectation`
- **context recipe**: task ごとに context pack の section と必要 field を定義する schema pack 内の設定
- **entity**: retrieval の検索起点。例: `character:A`, `policy:subsidy-x`
- **context pack**: `md-wiki context` が返す Markdown 形式の根拠パック
- **Source Trail**: context pack に含まれる根拠 page / source range の一覧
- **internal catalog**: `.md-wiki/catalog.json`。`md-wiki context` が使う内部 retrieval 索引
```

Update `manifest` term to mention both manifest and catalog:

```md
- **manifest**: `.md-wiki/manifest.json`。入力 root、再帰設定、source hash、生成ファイル hash を記録し、`add` の差分反映に使う管理ファイル
- **catalog**: `.md-wiki/catalog.json`。生成ページ、source range、entity、field、link graph を記録し、`context` の retrieval に使う内部索引
```

- [ ] **Step 2: Replace use cases with retrieval-first scenarios**

Edit `docs/要件定義/05-ユースケース.md`.

Replace the whole file with:

```md
# 5. ユースケース

## UC-01 Game narrative context pack
agent が「第 3 章後、キャラ A とキャラ B が再会し、読者の予想を裏切るイベントを書きたい」と依頼されたとき、`md-wiki context` で `twist-payoff` context pack を取得する。

```sh
md-wiki context \
  --wiki ./md-wiki \
  --schema schemas/game-narrative.yml \
  --task twist-payoff \
  --entity character:A \
  --entity character:B \
  --time after:chapter-3 \
  --query "再会イベントで読者の予想を裏切る" \
  --budget 20000
```

出力は `Hard Canon`、`Setup Already Planted`、`Reader Expectation`、`Hidden Alternative Reading`、`Payoff Candidates`、`Constraints`、`Source Trail` を含む Markdown context pack である。

## UC-02 Compile local Markdown corpus
ユーザーは Markdown corpus と schema pack を指定し、agent が探索しやすい artifacts を生成する。

```sh
md-wiki init ~/notes --schema schemas/game-narrative.yml -o ~/my-wiki
```

生成物には `fragments/`、`agent/pages/`、`agent/terms/`、`agent/fields/`、`.md-wiki/catalog.json` が含まれる。

## UC-03 Incremental update
Markdown corpus に `.md` を追加・変更・削除したあと、`md-wiki add` で compile artifacts と internal catalog を更新する。

```sh
md-wiki add -o ~/my-wiki --schema schemas/game-narrative.yml
```

## UC-04 Agent file-tool exploration
agent は `index.md`、`agent/index.md`、`agent/pages/`、`agent/terms/`、`agent/fields/` を通常の file/list/search/read ツールで辿り、必要な根拠ページを読む。

## UC-05 Missing evidence review
context recipe の required field が見つからない場合、context pack の `Missing Required Evidence` に不足項目が出る。人間は corpus または schema pack を修正できる。

## UC-06 Business/admin schema pack
将来、行政・業務用 schema pack により、制度説明、手続き回答、意思決定記録の context pack を同じ engine で生成する。
```

- [ ] **Step 3: Verify no stale `notes/` example remains in use cases**

Run:

```sh
rtk rg -n "notes/|Notebook|LLM が生成|入口ページ一覧" docs/要件定義/04-用語定義.md docs/要件定義/05-ユースケース.md
```

Expected: No stale `notes/` output path or LLM-generation wording appears. If `LLM` appears, it must be in the context of external agents, not `md-wiki` calling LLMs.

- [ ] **Step 4: Commit Task 2**

Run:

```sh
rtk git add docs/要件定義/04-用語定義.md docs/要件定義/05-ユースケース.md
rtk git commit -m "Define retrieval compiler terms and use cases"
```

Expected: A commit is created.

## Task 3: Specify Schema Input and Retrieval CLI

**Files:**
- Modify: `docs/要件定義/07-入力.md`
- Modify: `docs/要件定義/08-機能要件.md`
- Modify: `docs/要件定義/12-エラー処理.md`

- [ ] **Step 1: Add CLI forms to input requirements**

Edit `docs/要件定義/07-入力.md`.

Replace the CLI form block with:

```md
md-wiki init [INPUT] [--no-recursive] [--schema <YAML>] [--out <DIR>]
md-wiki add [PATH] [--schema <YAML>] [--out <DIR>]
md-wiki context --wiki <DIR> --schema <YAML> --task <TASK> [--entity <TYPE:ID>]... [--time <EXPR>] [--query <TEXT>] [--budget <CHARS>]
```

Add this row to the options table:

```md
| `--schema <YAML>` | なし | field 抽出と context recipe を定義する schema pack。指定時は `init` / `add` で field catalog と `.md-wiki/catalog.json` に schema 抽出結果を含める。`context` では必須 |
```

Add this subsection after the CLI table:

```md
### `context` 入力

`context` は生成済み wiki を対象に Markdown context pack を stdout に返す。

| 引数 / オプション | 必須 | 説明 |
|---|---:|---|
| `--wiki <DIR>` | yes | `.md-wiki/catalog.json` を持つ生成済み wiki |
| `--schema <YAML>` | yes | compile 時と同じ schema pack |
| `--task <TASK>` | yes | schema pack の `contexts` に定義された context recipe |
| `--entity <TYPE:ID>` | no | retrieval の明示起点。複数指定可 |
| `--time <EXPR>` | no | frontmatter または schema field の明示 metadata に対する一致条件 |
| `--query <TEXT>` | no | title、heading、tag、field text に対する文字列検索 |
| `--budget <CHARS>` | no | context pack の文字数上限。省略時は context recipe の `default_budget_chars` |
```

- [ ] **Step 2: Add schema pack input requirements**

Append to `docs/要件定義/07-入力.md`:

```md
## 7.6 Schema pack YAML

schema pack は外部 YAML ファイルである。Rust 本体にドメイン固有の retrieval logic を直書きしない。

必須 top-level field:

| field | 型 | 説明 |
|---|---|---|
| `id` | string | schema pack ID |
| `version` | integer | schema pack schema version。v1 は `1` |
| `fields` | map | field 抽出定義 |
| `contexts` | map | context recipe 定義 |

任意 top-level field:

| field | 型 | 説明 |
|---|---|---|
| `description` | string | schema pack の説明 |
| `entity_types` | string[] | 使用できる entity type の一覧 |

v1 の field source は `frontmatter` と `heading` に対応する。v1 では scoring DSL、任意コード実行、外部コマンド実行を許可しない。
```

- [ ] **Step 3: Add functional requirements for schema and context**

Edit `docs/要件定義/08-機能要件.md`.

Add these sections after `FR-15 増築性`:

```md
## FR-17 Schema pack 読み込み

`--schema <YAML>` が指定された場合、schema pack を読み込み、`id`、`version`、`fields`、`contexts` を検証する。

- `version` は v1 では `1` のみ対応
- `fields` の各 field は 1 個以上の `sources` を持つ
- source は `frontmatter: <path>` または `heading: <text>` を持つ
- `contexts` の各 context は 1 個以上の `sections` を持つ
- section は `fields` または `kind: sources` を持つ
- 未定義 field を参照する context はエラー
- schema pack 内の任意コード実行は不可

## FR-18 Schema field 抽出

schema pack が指定された場合、取り込んだ Markdown から schema field を抽出する。

- `frontmatter: narrative.setup` は YAML frontmatter の nested path を参照する
- `heading: Canon` は該当見出し配下の本文範囲を field として抽出する
- 同一 field に複数 source がある場合、原文出現順で統合する
- field 抽出結果は source path と source range を持つ
- 抽出できない field は空として扱い、compile 自体は失敗しない

## FR-19 Internal catalog

`init` / `add` は `.md-wiki/catalog.json` を生成する。

catalog は `context` CLI の内部 retrieval 索引であり、ユーザーが直接編集しない。

catalog は少なくとも以下を持つ:

- schema id / version
- generated page path
- source path
- source range
- title
- doc type
- entities
- tags
- headings
- extracted fields
- outgoing links
- backlinks

## FR-20 Field catalog

schema pack が指定された場合、`agent/fields/` に field ごとの Markdown catalog を生成する。

- `agent/fields/index.md` は field 一覧を持つ
- `agent/fields/<field>.md` はその field が抽出された pages / source ranges を列挙する
- field catalog は人間と agent が file tool で確認できる Markdown とする

## FR-21 Context pack 生成

`md-wiki context` は schema pack の context recipe と `.md-wiki/catalog.json` を使い、Markdown context pack を stdout に返す。

候補収集は以下の順で行う:

1. `--entity` に一致する pages
2. 一致 pages から wikilink / backlink 1 hop
3. `--task` の context recipe が参照する fields を持つ pages
4. `--query` に title / heading / tag / field text が文字列一致する pages
5. `--time` が明示 metadata に一致する pages

context pack は context recipe の section order に従う。`required: true` の section に対応する field が見つからない場合、`Missing Required Evidence` に不足項目を出す。

## FR-22 Context pack budget

`context` は `--budget` または context recipe の `default_budget_chars` を上限として Markdown context pack を生成する。

- budget 超過時は本文抜粋を削る
- `Source Trail` は常に残す
- YAML frontmatter metadata は残す
- 何も見つからない場合も、検索条件、missing evidence、空の Source Trail を持つ context pack を返す
```

- [ ] **Step 4: Add error handling**

Edit `docs/要件定義/12-エラー処理.md`.

Add under input validation:

```md
- `context` で `--wiki`、`--schema`、`--task` が欠ける → エラー終了
- `--schema` の YAML が存在しない、読めない、不正、または必須 field を欠く → エラー終了
- `--task` が schema pack の `contexts` に存在しない → エラー終了
- context recipe が未定義 field を参照する → エラー終了
- `.md-wiki/catalog.json` が無い、読めない、schema id / version が一致しない → エラー終了し、`init` / `add` の再実行を促す
```

Add under generation/runtime handling:

```md
- `context` で entity が見つからない → 正常終了し、context pack の warning section に出す
- required field が見つからない → 正常終了し、`Missing Required Evidence` に出す
- budget 超過 → 正常終了し、本文量を削って `Source Trail` を保持する
```

- [ ] **Step 5: Verify no future-only scoring is included**

Run:

```sh
rtk rg -n "scoring DSL|embedding|類似検索|LLM 呼び出し|任意コード" docs/要件定義/07-入力.md docs/要件定義/08-機能要件.md docs/要件定義/12-エラー処理.md
```

Expected: These terms only appear as explicit non-goals or forbidden behavior, not implemented v1 behavior.

- [ ] **Step 6: Commit Task 3**

Run:

```sh
rtk git add docs/要件定義/07-入力.md docs/要件定義/08-機能要件.md docs/要件定義/12-エラー処理.md
rtk git commit -m "Specify schema packs and context retrieval"
```

Expected: A commit is created.

## Task 4: Update Output, Constraints, and Design Decisions

**Files:**
- Modify: `docs/要件定義/09-非機能要件.md`
- Modify: `docs/要件定義/10-出力要件.md`
- Modify: `docs/要件定義/11-制約.md`
- Modify: `docs/要件定義/15-確定済み仕様.md`
- Modify: `docs/要件定義/16-設計判断.md`

- [ ] **Step 1: Reframe non-functional requirements**

Edit `docs/要件定義/09-非機能要件.md`.

Add these bullets near the top:

```md
- **小コンテキスト agent 適性**: agent が生成 wiki 全体を読まずに、索引、catalog、field、context pack から必要な根拠だけを取得できること
- **根拠追跡性**: context pack の各項目は、可能な限り generated page path と source range を追跡できること
- **retrieval 決定性**: 同一入力、同一 schema、同一 `context` command から同一 context pack が生成されること
- **透明性**: `md-wiki context` の候補は Markdown artifacts と `.md-wiki/catalog.json` から説明可能であり、LLM の不可視推論に依存しないこと
```

Keep the existing page-size text for now, but append:

```md
通常 compile artifacts の 40,000 文字超は当面 warn とする。`context` が返す context pack は指定 budget を超えてはならない。
```

- [ ] **Step 2: Update output structure**

Edit `docs/要件定義/10-出力要件.md`.

Update the tree to include:

```text
├── agent/
│   ├── index.md
│   ├── pages/
│   │   ├── index.md
│   │   └── page-001.md
│   ├── terms/
│   │   ├── index.md
│   │   └── page-001.md
│   └── fields/
│       ├── index.md
│       └── <field>.md
```

Update `.md-wiki/` to include:

```text
├── .md-wiki/
│   ├── manifest.json
│   └── catalog.json
```

Replace the `## index.md` bullet that says root `index.md` directly lists entry pages with:

```md
- 各セクションへの導線: `fragments/_index.md`, `agent/index.md`, `tags/index.md`, `headings/index.md`, `links/index.md`, `_unresolved.md`
- ルート `index.md` は全ノートを直接列挙しない。全ノートへの到達は `fragments/_index.md` と `agent/pages/` に委譲する
```

Add a `## Context pack 出力` section:

```md
## Context pack 出力

`md-wiki context` は生成 wiki ディレクトリへファイルを書き込まず、Markdown context pack を stdout に出力する。

context pack は以下を持つ:

- YAML frontmatter `md_wiki_context`
- context title
- schema context recipe に従う sections
- `Missing Required Evidence`（不足がある場合）
- `Source Trail`

context pack は `--budget` または context recipe の `default_budget_chars` を超えない。
```

- [ ] **Step 3: Update constraints**

Edit `docs/要件定義/11-制約.md`.

Add:

```md
- LLM 呼び出しを行わない
- embedding または vector database に依存しない
- 自然言語の意味的類似検索を行わない
- 原文から未明示の意味、感情、関係性、伏線を推論しない
- schema pack 内の任意コード実行を許可しない
- `context` は生成 wiki を破壊・変更しない
```

- [ ] **Step 4: Update confirmed specs**

Edit `docs/要件定義/15-確定済み仕様.md`.

Add rows:

```md
| 主目的 | 小さいコンテキストの agent 向け retrieval compiler | [§1](01-概要.md) |
| 二層構造 | `init` / `add` による compile layer と `context` による retrieval layer | [FR-21](08-機能要件.md#fr-21-context-pack-生成) |
| schema pack | 外部 YAML で field 抽出と context recipe を定義する。v1 は scoring DSL なし | [§7.6](07-入力.md#76-schema-pack-yaml), [FR-17](08-機能要件.md#fr-17-schema-pack-読み込み) |
| internal catalog | `.md-wiki/catalog.json` を `context` retrieval 用の内部索引として生成する | [FR-19](08-機能要件.md#fr-19-internal-catalog) |
| context pack | `md-wiki context` は Markdown を stdout に返す。LLM 呼び出しや要約は行わない | [FR-21](08-機能要件.md#fr-21-context-pack-生成) |
```

- [ ] **Step 5: Add design decisions**

Append to `docs/要件定義/16-設計判断.md`:

```md
## Retrieval compiler として再定義した理由
小さいコンテキストの agent では、生成 wiki 全体を読むことも、巨大な root index を読むことも現実的ではない。必要なのは、人間向け wiki そのものではなく、agent が目的に応じて根拠断片を集められる compile artifacts と context pack である。

## 静的 artifacts と retrieval CLI を両方持つ理由
静的 Markdown artifacts は透明性、レビュー性、デバッグ性を提供する。一方、agent が毎回 file tool だけで探索すると手順が長くなり、小さい LLM ほど失敗しやすい。`context` CLI は artifacts と internal catalog を使って、決定的な context pack を短い手順で返す。

## schema pack を外部 YAML にした理由
ゲーム、行政、業務などの domain field は Rust 本体に固定すべきではない。外部 YAML により、core engine は field 抽出と context recipe 実行に集中できる。

## `.md-wiki/catalog.json` を許容する理由
生成 wiki の可読成果物は Markdown を基本とするが、`context` CLI が毎回全 Markdown を読み直すと遅く不安定になる。内部 retrieval 用に JSON catalog を持つことで、速度、決定性、検証性を確保する。

## 意味推論をしない理由
`md-wiki` が「これは伏線である」「この関係は親子に近い」と推論すると、決定性と説明可能性が崩れる。未明示の意味判断は外部 agent / LLM に任せ、`md-wiki` は明示された field と source trail を提供する。
```

- [ ] **Step 6: Commit Task 4**

Run:

```sh
rtk git add docs/要件定義/09-非機能要件.md docs/要件定義/10-出力要件.md docs/要件定義/11-制約.md docs/要件定義/15-確定済み仕様.md docs/要件定義/16-設計判断.md
rtk git commit -m "Document retrieval compiler outputs and constraints"
```

Expected: A commit is created.

## Task 5: Update Acceptance Criteria and Verification Matrix

**Files:**
- Modify: `docs/要件定義/13-受け入れ基準.md`
- Modify: `docs/要件定義/17-継続検証と品質スコア.md`

- [ ] **Step 1: Fix AC-12 contradiction**

Edit `docs/要件定義/13-受け入れ基準.md`.

Replace AC-12 with:

```md
## AC-12 index.md
ルートの `index.md` にノート数・断片数・タグ数・未解決数のサマリと、`fragments/_index.md`、`agent/index.md`、`tags/index.md`、`headings/index.md`、`links/index.md`、`_unresolved.md` への導線が含まれる。ルート `index.md` は全ノートや全入口ページを直接列挙しない。断片数は h2/h3 断片ページの合計で、入口ページ・殻ページは含まない。
```

- [ ] **Step 2: Add retrieval acceptance criteria**

Append to `docs/要件定義/13-受け入れ基準.md`:

```md
## AC-24 schema pack 読み込み
`--schema` に valid な schema pack YAML を指定すると `init` / `add` / `context` が schema id、version、fields、contexts を読み込める。不正 YAML、必須 field 欠落、未定義 field 参照はエラーになる。

## AC-25 internal catalog
`md-wiki init --schema <YAML>` は `.md-wiki/catalog.json` を生成する。catalog には schema id / version、generated page path、source path、source range、entities、tags、headings、extracted fields、outgoing links、backlinks が含まれる。

## AC-26 field catalog
schema pack が指定された場合、`agent/fields/index.md` と `agent/fields/<field>.md` が生成される。field catalog は該当 field を持つ generated page と source range を列挙する。

## AC-27 context pack 生成
`md-wiki context --wiki <DIR> --schema <YAML> --task <TASK>` は Markdown context pack を stdout に出す。出力は YAML frontmatter `md_wiki_context`、context title、schema の section order に従う本文、`Source Trail` を含む。

## AC-28 required field 欠落
context recipe の `required: true` field が見つからない場合、`context` は正常終了し、`Missing Required Evidence` に不足 field と対象条件を出す。

## AC-29 context budget
`context` の出力文字数は `--budget` または context recipe の `default_budget_chars` を超えない。budget 超過時も YAML frontmatter と `Source Trail` は保持される。

## AC-30 retrieval 決定性
同一入力、同一 schema、同一 `context` command では byte-identical な context pack が出力される。

## AC-31 no inference / no network
`context` は LLM、embedding、network、外部 API、schema pack 内の任意コード実行に依存しない。候補収集は catalog、明示 metadata、文字列一致、wikilink / backlink 1 hop に限定される。
```

- [ ] **Step 3: Update verification matrix**

Edit `docs/要件定義/17-継続検証と品質スコア.md`.

Add rows to the AC table:

```md
| AC-24 | schema YAML を読み込み、valid / invalid / 未定義 field 参照を検証する。 | `tests/schema_pack.rs` |
| AC-25 | `init --schema` が `.md-wiki/catalog.json` を生成し、必須 catalog fields を含む。 | `tests/acceptance.rs::schema_init_generates_internal_catalog` |
| AC-26 | `agent/fields/index.md` と field ごとの catalog が生成される。 | `tests/acceptance.rs::schema_field_catalogs_are_generated` |
| AC-27 | `context` が Markdown context pack を stdout に返し、frontmatter、sections、Source Trail を含む。 | `tests/e2e.rs::context_outputs_markdown_pack` |
| AC-28 | required field 欠落が `Missing Required Evidence` に出る。 | `tests/e2e.rs::context_reports_missing_required_evidence` |
| AC-29 | `context` 出力が budget を超えず、Source Trail を保持する。 | `tests/e2e.rs::context_respects_budget` |
| AC-30 | 同一 `context` command の出力が byte-identical。 | `tests/e2e.rs::context_output_is_deterministic` |
| AC-31 | `context` が LLM、embedding、network に依存しない。 | 依存レビュー + offline gate |
```

Add quality checks:

```md
| `schema-catalog` | 10 | schema 指定時に catalog と field catalog が生成され、必須項目が揃う |
| `context-pack` | 15 | game narrative fixture から valid な Markdown context pack が生成される |
| `context-budget` | 10 | context pack が指定 budget を超えない |
| `context-determinism` | 10 | 同一 context command が byte-identical |
```

- [ ] **Step 4: Verify acceptance numbering**

Run:

```sh
rtk rg -n "^## AC-" docs/要件定義/13-受け入れ基準.md
```

Expected: AC-01 through AC-31 appear once each, in order.

- [ ] **Step 5: Commit Task 5**

Run:

```sh
rtk git add docs/要件定義/13-受け入れ基準.md docs/要件定義/17-継続検証と品質スコア.md
rtk git commit -m "Add retrieval compiler acceptance criteria"
```

Expected: A commit is created.

## Task 6: Move Future-Scope and Implementation-Track Items

**Files:**
- Modify: `docs/要件定義/14-将来拡張.md`
- Modify: `docs/要件定義/18-巨大Markdown対応完了条件.md`
- Modify: `docs/要件定義/19-巨大Markdown対応設計と機械検証.md`
- Modify: `docs/要件定義/20-ツール使用型エージェント向け探索仕様と設計.md`
- Modify: `docs/要件定義/21-agentic-search実装作業計画.md`

- [ ] **Step 1: Add future scope items**

Edit `docs/要件定義/14-将来拡張.md`.

Add:

```md
## Retrieval 拡張
- schema pack の scoring DSL
- JSON context pack 出力
- 複数 schema pack の同時適用
- domain schema pack marketplace / registry
- semantic search
- embedding / vector database 連携
- LLM による optional summary layer
- context pack の差分更新
```

- [ ] **Step 2: Add implementation-track notice to large Markdown docs**

At the top of each of these files, immediately after the first heading:

- `docs/要件定義/18-巨大Markdown対応完了条件.md`
- `docs/要件定義/19-巨大Markdown対応設計と機械検証.md`
- `docs/要件定義/20-ツール使用型エージェント向け探索仕様と設計.md`
- `docs/要件定義/21-agentic-search実装作業計画.md`

Add:

```md
> Status: 本章は retrieval compiler 方針の implementation track である。`docs/要件定義/01-概要.md`〜`17-継続検証と品質スコア.md` の中核仕様を正とし、本章は巨大 Markdown / agentic search 実装を進める際に再整理する。
```

- [ ] **Step 3: Verify future-only items are not in core requirements as mandatory v1 behavior**

Run:

```sh
rtk rg -n "scoring DSL|semantic search|embedding|vector database|JSON context" docs/要件定義/01-概要.md docs/要件定義/02-スコープ.md docs/要件定義/07-入力.md docs/要件定義/08-機能要件.md docs/要件定義/13-受け入れ基準.md
```

Expected: These appear only as non-goals, prohibitions, or not at all.

- [ ] **Step 4: Commit Task 6**

Run:

```sh
rtk git add docs/要件定義/14-将来拡張.md docs/要件定義/18-巨大Markdown対応完了条件.md docs/要件定義/19-巨大Markdown対応設計と機械検証.md docs/要件定義/20-ツール使用型エージェント向け探索仕様と設計.md docs/要件定義/21-agentic-search実装作業計画.md
rtk git commit -m "Separate future retrieval extensions from core scope"
```

Expected: A commit is created.

## Task 7: Final Consistency Review

**Files:**
- Review all files in `docs/要件定義/`

- [ ] **Step 1: Search for stale wiki-first language**

Run:

```sh
rtk rg -n "人間向け wiki|入口ページ一覧|notes/|AI に食わせ|LLM を使|要約生成|全ノートの列挙" docs/要件定義
```

Expected:

- `入口ページ一覧` should not appear as a root `index.md` requirement.
- `notes/` should not appear as an output path.
- `LLM` references should describe external agent behavior or non-goals, not `md-wiki` calling LLMs.
- `全ノートの列挙` may appear only to say root `index.md` must not do it.

- [ ] **Step 2: Search for conflicting page limit language**

Run:

```sh
rtk rg -n "40,000|hard limit|warn|警告" docs/要件定義/09-非機能要件.md docs/要件定義/10-出力要件.md docs/要件定義/13-受け入れ基準.md docs/要件定義/18-巨大Markdown対応完了条件.md docs/要件定義/20-ツール使用型エージェント向け探索仕様と設計.md
```

Expected:

- Core compile artifacts may still warn on page size over 40,000 until large Markdown implementation track replaces this.
- `context` output must obey budget.
- Large Markdown / agentic implementation tracks may still define hard limit completion criteria, but are labeled implementation tracks.

- [ ] **Step 3: Check Markdown diff health**

Run:

```sh
rtk git diff --check
```

Expected: no whitespace errors.

- [ ] **Step 4: Review all requirement diffs**

Run:

```sh
rtk git diff origin/main... -- docs/要件定義 docs/superpowers
```

Expected: The diff shows only documentation changes and no unrelated file modifications.

- [ ] **Step 5: Optional smoke test**

Run if no Rust code has changed:

```sh
rtk cargo test
```

Expected: Existing tests pass. If this is too slow for a docs-only branch, record that it was skipped because no code changed.

- [ ] **Step 6: Commit any final cleanup**

If Steps 1-4 found stale language and fixes were made:

```sh
rtk git add docs/要件定義 docs/superpowers
rtk git commit -m "Clean up retrieval compiler requirement consistency"
```

If no fixes were needed, do not create an empty commit.

## Self-Review Checklist

- Spec coverage: This plan covers product definition, primary use case, schema pack YAML, compile artifacts, CLI, retrieval behavior, context pack format, non-goals, acceptance criteria, and migration from the approved design.
- Scope control: This plan intentionally does not implement Rust behavior. It only updates requirements docs so implementation can be planned afterward.
- Placeholders: No task uses TBD/TODO language. Each task names exact files, exact snippets, verification commands, and commit messages.
- Type consistency: Terms are consistently named as schema pack, field, context recipe, entity, context pack, and internal catalog.
