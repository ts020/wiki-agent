# md-wiki Retrieval Compiler 設計

## 1. プロダクト定義

`md-wiki` は、ローカル Markdown corpus を小さいコンテキストの agent が探索・引用・再利用できる形へ変換する、オフラインの retrieval compiler である。

`md-wiki` は 2 つの層を持つ。

- **Compile layer**: `init` / `add` で Markdown corpus を決定的な探索用 artifacts へ変換する。
- **Retrieval layer**: `context` CLI で、task / entity / query に応じた Markdown context pack を返す。

`md-wiki` は LLM 呼び出し、embedding、未明示の意味推論、最終文章生成を行わない。Markdown の frontmatter、見出し、明示リンク、タグ、source range、schema pack YAML で定義された field / context recipe を集約し、外部 agent が推論できる根拠パックを作る。

## 2. 主要ユースケース

最初の代表ユースケースは、ゲーム脚本の `twist-payoff` context pack 生成である。

ユーザーは agent に「第 3 章後、キャラ A とキャラ B が再会し、地味な積み上げの上で読者の予想を裏切るイベントを書きたい」と依頼する。agent は `md-wiki context` を呼び、イベント本文を書く前に必要な設定・伏線・制約を context pack として取得する。

例:

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

このユースケースで必要な context pack は、単なる検索結果ではない。イベントを書くために破ってはいけない事実、過去に積まれた小さな描写、読者が自然に信じている前提、後から別解釈できる材料、今回回収できる候補、明かしてはいけない制約をまとめる。

想定セクション:

- `Hard Canon`
- `Setup Already Planted`
- `Reader Expectation`
- `Hidden Alternative Reading`
- `Payoff Candidates`
- `Constraints`
- `Missing Required Evidence`
- `Source Trail`

## 3. 中核概念

- **Corpus**: 入力となる Markdown ファイル群。
- **Compile artifacts**: `init` / `add` が生成する探索用 Markdown と内部索引。
- **Schema pack YAML**: corpus から何を field として収集し、task ごとに context pack をどう組むかを定義する外部 YAML。
- **Field**: `canon`、`setup`、`expectation` のような、schema が定義する収集単位。
- **Context recipe**: `twist-payoff` のような task に対して、どの field をどのセクションへ出すかを定義する recipe。
- **Entity**: `character:A`、`policy:subsidy-x` のような検索起点。entity type は schema pack で定義できる。
- **Source range**: 生成ページや context pack 項目が、どの入力 Markdown のどの範囲に由来するかを示す情報。
- **Context pack**: agent が LLM に渡せる、task 向けの Markdown 根拠パック。
- **Internal catalog**: `md-wiki context` が高速・安定に retrieval するための `.md-wiki/catalog.json`。

## 4. Schema Pack YAML

schema pack は最初から外部 YAML として定義する。Rust 本体にゲームや行政などのドメイン固有ロジックを直書きしない。

schema pack は 2 系統を管理する。

1. **収集・分類**: Markdown からどの field を拾うか。
2. **検索・context pack recipe**: task ごとにどの field をどう並べるか。

v1 では scoring DSL は持たない。検索優先度は core engine の固定ルールで扱い、schema pack は field mapping と context recipe に集中する。

例:

```yaml
id: game-narrative
version: 1
description: Game narrative context pack schema.

entity_types:
  - character
  - relationship
  - event
  - location
  - faction

fields:
  canon:
    label: Canon
    sources:
      - frontmatter: canon
      - heading: Canon
      - heading: 確定設定

  setup:
    label: Setup
    sources:
      - frontmatter: narrative.setup
      - heading: Setup
      - heading: 伏線

  expectation:
    label: Reader Expectation
    sources:
      - frontmatter: narrative.expectation
      - heading: Reader Expectation
      - heading: 読者の想定

  hidden_reading:
    label: Hidden Alternative Reading
    sources:
      - frontmatter: narrative.hidden_reading
      - heading: Hidden Reading
      - heading: 別解釈

  payoff:
    label: Payoff Candidate
    sources:
      - frontmatter: narrative.payoff
      - heading: Payoff
      - heading: 回収候補

  do_not_break:
    label: Constraints
    sources:
      - frontmatter: do_not_break
      - heading: Do Not Break
      - heading: 禁止事項

contexts:
  twist-payoff:
    title: Twist / Payoff Context
    default_budget_chars: 20000
    sections:
      - title: Hard Canon
        fields: [canon]
        required: true
      - title: Setup Already Planted
        fields: [setup]
      - title: Reader Expectation
        fields: [expectation]
      - title: Hidden Alternative Reading
        fields: [hidden_reading]
      - title: Payoff Candidates
        fields: [payoff]
      - title: Constraints
        fields: [do_not_break]
      - title: Source Trail
        kind: sources
```

## 5. 生成 Artifacts

`init` / `add` は、人間と agent が読める Markdown artifacts と、CLI が内部利用する JSON catalog を生成する。

想定構造:

```text
md-wiki/
├── index.md
├── fragments/
│   └── <rel>/
│       ├── index.md
│       └── <section>.md
├── headings/
├── links/
├── tags/
├── _unresolved.md
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
│       ├── canon.md
│       ├── setup.md
│       └── <field>.md
└── .md-wiki/
    ├── manifest.json
    └── catalog.json
```

`agent/pages/`、`agent/terms/`、`agent/fields/` は Markdown の探索入口である。`catalog.json` はユーザーが編集するものではなく、`context` CLI が使う内部索引である。

`catalog.json` には少なくとも以下を含める。

- 生成ページ path
- title
- source path
- source range
- doc type
- entities
- tags
- headings
- schema field 抽出結果
- outgoing links
- backlinks

## 6. CLI

Compile layer:

```sh
md-wiki init [INPUT] \
  --schema schemas/game-narrative.yml \
  --out ./md-wiki

md-wiki add [PATH] \
  --schema schemas/game-narrative.yml \
  --out ./md-wiki
```

Retrieval layer:

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

`context` は Markdown を stdout に返す。JSON 出力は v1 の第一級要件にしない。ただし内部 catalog は JSON を許容する。

## 7. Retrieval 挙動

`md-wiki context` は決定的なルールで候補を集める。

候補収集:

1. `--entity` に一致するページを集める。
2. 一致ページから wikilink / backlink を 1 hop だけ展開する。
3. `--task` の context recipe に必要な field を持つページを優先する。
4. `--query` は title、heading、tag、field text に対する文字列検索として使う。
5. `--time` は frontmatter や schema field の明示 metadata に対する一致条件として使う。
6. budget 内に収まるように、required section から順に本文を詰める。

やらないこと:

- 意味的類似検索
- embedding
- LLM 要約
- 原文からの自動伏線判定
- 未明示の関係性や感情の推論

失敗時挙動:

- entity が見つからない場合は、warning section を出す。
- required field が見つからない場合は、`Missing Required Evidence` に出す。
- budget 超過時は、本文抜粋を削り、`Source Trail` は残す。
- schema が不正な場合はエラー終了する。
- `catalog.json` が無い、または古い場合は、`init` / `add` の再実行を促すエラーにする。

## 8. Context Pack 形式

context pack は Markdown first とする。先頭に機械可読な YAML frontmatter を持ち、本文は agent がそのまま LLM に渡せる形にする。

例:

```md
---
md_wiki_context:
  schema: game-narrative
  task: twist-payoff
  budget_chars: 20000
  entities:
    - character:A
    - character:B
  query: 再会イベントで読者の予想を裏切る
  generated_from:
    - fragments/characters/a/index.md
    - fragments/events/chapter-2-betrayal/index.md
---

# Twist / Payoff Context

## Hard Canon

- A は第 2 章で B に裏切られた。
  Source: fragments/characters/a/index.md#canon

## Setup Already Planted

- B は過去イベントで敵の名前だけを避けて話していた。
  Source: fragments/events/chapter-1-warning/index.md#setup

## Missing Required Evidence

- `canon` for `character:B` was not found.

## Source Trail

- fragments/characters/a/index.md
- fragments/events/chapter-2-betrayal/index.md
```

`Source Trail` は必須である。context pack の各項目は、可能な限り source path と source range を持つ。

## 9. 非目標

v1 の非目標:

- LLM 呼び出し
- embedding または vector database
- 自然言語の意味的類似検索
- 原文からの自動意味づけ
- 自動要約
- 最終文章生成
- Rust 本体へのドメイン固有 retrieval logic の直書き
- schema pack 内の任意コード実行

## 10. 受け入れ基準ドラフト

この設計を仕様へ落とすとき、最低限以下を受け入れ基準にする。

- schema YAML を読み込み、field と context recipe を検証できる。
- 不正な schema YAML は明確なエラーで失敗する。
- `init` / `add --schema` が `.md-wiki/catalog.json` を生成する。
- `catalog.json` に page、entity、field、source range、link graph が含まれる。
- `agent/fields/` に schema field ごとの Markdown catalog が生成される。
- `md-wiki context --task twist-payoff` が Markdown context pack を stdout に返す。
- context pack は YAML frontmatter metadata を持つ。
- context pack は schema の section order に従う。
- required field が見つからない場合、`Missing Required Evidence` に出る。
- budget を超えない。
- `Source Trail` が常に出る。
- 同一入力、同一 schema、同一 command で byte-identical な出力になる。
- LLM、embedding、network に依存しない。

## 11. 既存仕様からの移行方針

現行 v1 wiki 仕様は compile layer の baseline として残す。ただし、新しいプロダクト定義では「人間向け wiki」ではなく「agent 向け retrieval compiler」を主目的に置く。

現在の `docs/要件定義/18-巨大Markdown対応完了条件.md`、`19-巨大Markdown対応設計と機械検証.md`、`20-ツール使用型エージェント向け探索仕様と設計.md`、`21-agentic-search実装作業計画.md` は、この設計の implementation track として整理し直す。

以下は設計作成時点で優先的に解消すべきだった矛盾である。要件定義への移行後は、root `index.md` と AC-12 / AC-22 の矛盾は解消済み、40,000 文字制限と `fragment: false` の hard limit 化は巨大 Markdown / agentic search implementation track、agentic / large Markdown 要件の実装検証は計画済み coverage として扱う。

- `index.md` に入口ページ一覧を出すのか、`fragments/_index.md` へ委譲するのか。
- 40,000 文字超を warn で継続するのか、agentic retrieval では hard limit とするのか。
- `fragment: false` を常に尊重するのか、context budget を超える場合は分割を優先するのか。
- AC-12 と AC-22 の root index 仕様の矛盾。
- agentic / large Markdown 要件が v1 acceptance criteria に統合されていない問題。

移行時は、先にこの設計を上位方針として合意し、その後に `docs/要件定義/` を再編する。再編後の正は `docs/要件定義/` であり、本設計 doc は移行判断の根拠として保持する。
