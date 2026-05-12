# 21. Agentic Search 実装作業計画

本章は [20. ツール使用型エージェント向け探索仕様と設計](20-ツール使用型エージェント向け探索仕様と設計.md) を実装するための作業計画である。各タスクは、人間の目視判断ではなく、コマンド、テスト名、JSON report、生成ファイル検査で完了を判定できる形にする。

## 21.1 完了定義

Agentic Search 対応は、以下の通常 gate がすべてゼロ終了し、JSON report が満点になった時点で通常完了とする。

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo run --quiet --example large_md_gate -- \
  --mode normal \
  --work-dir target/large-md-gate \
  --report target/large-md-gate/report.json \
  --min-score 100
cargo run --quiet --example agentic_search_gate -- \
  --mode normal \
  --work-dir target/agentic-search-gate \
  --report target/agentic-search-gate/report.json \
  --min-score 100
```

`target/agentic-search-gate/report.json` は以下を満たす。

- `schema_version == 1`
- `mode == "normal"`
- `passed == true`
- `score == max_score`
- `summary.oversized_pages == 0`
- `summary.broken_local_links == 0`
- `summary.unresolved_links == expected_unresolved_links`
- `summary.byte_identical_rerun == true`
- `checks[].passed` が全件 `true`

Heavy gate は通常完了後に明示実行する。main branch、nightly、または手動 CI の完了条件とする。

```sh
cargo run --quiet --example agentic_search_gate -- \
  --mode heavy \
  --work-dir target/agentic-search-heavy \
  --report target/agentic-search-heavy/report.json \
  --min-score 100 \
  --require-resource-budget
```

## 21.2 共通ルール

各タスクは以下を守る。

- 入力 Markdown は変更しない
- 外部 API、AI、embedding、ネットワークを使わない
- 生成物は Markdown と YAML frontmatter に留める
- 既存 AC-01〜AC-23 の挙動を壊さない
- 40,000 文字を超える `.md` を生成しない
- 同一入力・同一 option で byte-identical な出力にする
- 新しい挙動には focused test または gate check を追加する

各タスク完了時は、少なくとも以下を通す。

```sh
cargo fmt --check
cargo test
```

共有 module、CLI、renderer、link resolver、scanner を触ったタスクでは以下も通す。

```sh
cargo clippy -- -D warnings
```

## 21.3 タスク一覧

| ID | タスク | 依存 | 主な成果物 | 機械完了条件 |
|---|---|---|---|---|
| AG-00 | `agentic_search_gate` skeleton | なし | `src/agentic_search_gate.rs`, `examples/agentic_search_gate.rs` | report schema と全 check ID を出力できる |
| AG-01 | agentic fixture generator | AG-00 | deterministic fixtures | fixture hash と byte-identical rerun を検査できる |
| AG-02 | `InputClassifier` | AG-00 | regular / large / skipped 分類 | 1 MiB 超 `.md` が skip されず large に分類される |
| AG-03 | regular `PagePlan` adapter | AG-02 | 既存 fragment から `PagePlan` 生成 | 既存 h2 / h3 / fragment:false の計画が検査できる |
| AG-04 | `PageRegistry` | AG-03 | 全 page plan 統合 registry | path 重複、kind、char budget を検査できる |
| AG-05 | `MetadataRenderer` | AG-04 | 全 page kind の `md_wiki` metadata | YAML parse と必須 field 検査が通る |
| AG-06 | `IndexPager` 統合 | AG-04 | paged headings / links / tags / unresolved / `_index` | 全 index が 40,000 文字以内 |
| AG-07 | agent guide / catalog / term index | AG-05, AG-06 | `agent/index.md`, `agent/pages/`, `agent/terms/` | entrypoint / catalog / term-index checks が通る |
| AG-08 | large path の CLI 統合 | AG-02, AG-04, AG-05 | 1 MiB 超入力の通常 CLI 取り込み | 2〜5 MiB fixtures が CLI で生成完了 |
| AG-09 | shell / leaf 解像度 link graph | AG-04, AG-08 | wikilink / links / backlinks 拡張 | link graph と unresolved checks が通る |
| AG-10 | hard limit enforcement | AG-05, AG-06, AG-08 | 40,000 文字超 0 の保証 | oversized page 0、planning failure test |
| AG-11 | simulated agent query checks | AG-07, AG-09 | query fixture runner | 全 query が期待 leaf / index に到達 |
| AG-12 | `scripts/verify.sh` / CI 統合 | AG-11 | normal gate 常時実行 | verify script が agentic gate を含む |
| AG-13 | heavy gate | AG-12 | heavy fixtures と resource metrics | heavy report が満点 |
| AG-14 | docs / README 更新 | AG-12 | 利用方法と互換性説明 | docs tests と generated wiki smoke が通る |

## 21.4 AG-00 `agentic_search_gate` skeleton

### 作業

- `src/agentic_search_gate.rs` を追加する
- `examples/agentic_search_gate.rs` を追加する
- `GateMode`、`GateOptions`、`GateReport`、`GateSummary`、`GateCheck` を定義する
- 20.12 の check ID をすべて report に含める
- `--mode normal|heavy`、`--work-dir`、`--report`、`--min-score`、`--require-resource-budget`、`--fixture-bytes` を parse する

### 機械完了条件

```sh
cargo run --quiet --example agentic_search_gate -- \
  --mode normal \
  --work-dir target/agentic-search-gate \
  --report target/agentic-search-gate/report.json \
  --min-score 0
```

上記がゼロ終了し、report が以下を満たす。

- `schema_version == 1`
- `mode == "normal"`
- `checks[].id` に以下が全て含まれる
  - `agentic-entrypoint`
  - `agentic-page-budget`
  - `agentic-metadata`
  - `agentic-catalog`
  - `agentic-term-index`
  - `agentic-navigation`
  - `agentic-routing`
  - `agentic-query-fixtures`
  - `agentic-traceability`
  - `agentic-link-graph`
  - `agentic-no-full-read`
  - `agentic-determinism`

追加テスト:

```sh
cargo test agentic_search_gate::tests::normal_report_contains_all_check_ids
```

## 21.5 AG-01 agentic fixture generator

### 作業

以下の deterministic fixture を `target/agentic-search-gate/fixture/` に生成する。

| Fixture | 内容 | 検査対象 |
|---|---|---|
| `agent-heading-lookup.md` | h1〜h6 と marker heading を含む | headings / terms routing |
| `agent-no-heading.md` | 見出しなしの段落集合 | forced split / exact phrase |
| `agent-links.md` | wikilink、通常 link、backlink 対象 | link graph |
| `agent-tags.md` | nested tags と aliases | tags / terms |
| `agent-unresolved.md` | 解決不能 wikilink | unresolved route |
| `agent-ambiguous.md` | 同名 heading 複数 | catalog disambiguation |
| `agent-single-line.md` | 改行なし巨大行 | byte window traceability |

### 機械完了条件

```sh
cargo test agentic_search_gate::tests::fixtures_are_deterministic
cargo test agentic_search_gate::tests::fixtures_cover_required_query_classes
```

Report 条件:

- `summary.fixtures >= 7`
- `summary.input_bytes > 0`
- `checks[id=agentic-determinism].passed == true`

## 21.6 AG-02 `InputClassifier`

### 作業

- scan 結果を `regular_markdown` / `large_markdown` / `skipped` に分類する
- 1 MiB 超の UTF-8 `.md` は skip せず `large_markdown` に送る
- NULL byte、invalid UTF-8、非 `.md` は `skipped` にする
- 既存 v1.2 互換の skip 挙動が必要な場合は明示 option に閉じ込める

### 機械完了条件

```sh
cargo test input_classifier
cargo test scan::tests::large_markdown_is_classified_not_skipped
```

Fixture 条件:

- 1 MiB + 1 byte の valid UTF-8 `.md` が `large_markdown`
- 1 MiB + 1 byte の valid UTF-8 `.txt` が `skipped`
- NULL byte を含む `.md` が `skipped`
- invalid UTF-8 `.md` が `skipped`

Report 条件:

- `checks[id=agentic-entrypoint]` の metrics に `skipped_due_to_size == 0`

## 21.7 AG-03 regular `PagePlan` adapter

### 作業

- 既存の `Node` / `FragmentTree` から `PagePlan` を作る adapter を追加する
- entry、h2 leaf、h3 shell、h3 child leaf、fragment:false entry を表現する
- 既存出力 path と slug 衝突解決を維持する

### 機械完了条件

```sh
cargo test regular_page_plan
cargo test fragment::tests
cargo test render::paths::tests
```

検査条件:

- h2 1 個の note で `entry` 1 件、`leaf` 1 件
- h2 複数の note で source 順の `prev` / `next`
- h3 再分割で `shell` 1 件、child `leaf` 複数
- `fragment: false` で 40,000 文字以下なら `entry` のみ
- 既存 AC-16〜AC-20 の acceptance tests が通る

## 21.8 AG-04 `PageRegistry`

### 作業

- regular / large の `PagePlan` を統合する `PageRegistry` を追加する
- output path、page kind、source path、source range、navigation、estimated char count を一元管理する
- path 重複、parent 不在、prev / next 不整合を registry build 時に検出する

### 機械完了条件

```sh
cargo test page_registry
```

検査条件:

- 同一 output path の二重登録で error
- `parent` が存在しない page で error
- `next.prev != self` で error
- `estimated_chars > 40_000` で planning error
- regular と large が混在する fixture で registry build が成功

## 21.9 AG-05 `MetadataRenderer`

### 作業

- entry / shell / leaf / paged_index / agent_guide / page_catalog / term_index に YAML frontmatter を出す
- `page_kind` ごとの必須 field を満たす
- metadata の navigation と本文 blockquote nav を一致させる

### 機械完了条件

```sh
cargo test metadata_renderer
cargo test acceptance::agentic_metadata_present_on_all_page_kinds
```

Gate 条件:

- `checks[id=agentic-metadata].passed == true`
- `checks[id=agentic-navigation].passed == true`

YAML 検査:

- 全対象 `.md` の先頭 frontmatter が parse できる
- `md_wiki.schema_version == 1`
- `md_wiki.output_path` が実 path と一致する
- leaf は `source`、`byte_ranges`、`line_ranges` を持つ

## 21.10 AG-06 `IndexPager` 統合

### 作業

- `_index.md`、`headings/`、`links/`、`tags/`、`_unresolved.md` を共通 `IndexPager` に通す
- 40,000 文字を超える前に `page-001.md`、`page-002.md` へ分割する
- manifest page から paged index pages へ到達できる

### 機械完了条件

```sh
cargo test index_pager
cargo test acceptance::large_indexes_are_paged_under_budget
```

Gate 条件:

- `checks[id=agentic-page-budget].passed == true`
- `summary.oversized_pages == 0`
- `summary.max_page_chars <= 40_000`

Fixture 条件:

- 50,000 headings で `headings/page-001.md` が生成される
- 50,000 links で `links/page-001.md` が生成される
- 大量 unresolved で `_unresolved` が page budget 内に分割される

## 21.11 AG-07 agent guide / catalog / term index

### 作業

- `agent/index.md` を生成する
- `agent/pages/index.md` と paged catalog を生成する
- `agent/terms/index.md` と paged term index を生成する
- root `index.md` から `agent/index.md` へリンクする

### 機械完了条件

```sh
cargo test agent_outputs
cargo test acceptance::agent_entrypoints_are_generated
```

Gate 条件:

- `checks[id=agentic-entrypoint].passed == true`
- `checks[id=agentic-catalog].passed == true`
- `checks[id=agentic-term-index].passed == true`
- `checks[id=agentic-routing].passed == true`

生成物条件:

- `agent/index.md` が存在する
- `agent/pages/index.md` が存在する
- `agent/terms/index.md` が存在する
- catalog に生成された全 `.md` が重複なく載る
- term index に title / tag / alias / heading / wikilink / link_label / file の fixture term が載る

## 21.12 AG-08 large path の CLI 統合

### 作業

- 通常 CLI から large Markdown planner を呼び出す
- 2〜5 MiB の no-heading、single-heading、single-line、code-block、table fixture を通常 CLI で生成できるようにする
- regular と large が同一入力ディレクトリに混在しても 1 つの wiki として出力する

### 機械完了条件

```sh
cargo test e2e::large_markdown_cli_generates_agentic_pages
cargo test e2e::regular_and_large_inputs_share_one_wiki
```

手動検査コマンド:

```sh
cargo run --quiet -- init target/agentic-search-gate/fixture -o target/agentic-search-cli-out
```

生成物条件:

- `target/agentic-search-cli-out/agent/index.md` が存在する
- 2 MiB 超 `.md` 由来の `fragments/<rel>/part-001.md` または section leaf が存在する
- 生成 `.md` の最大文字数が 40,000 以下
- valid UTF-8 2 MiB `.md` に対して `file exceeds 1 MiB, skipping` ログが出ない

## 21.13 AG-09 shell / leaf 解像度 link graph

### 作業

- `[[Note#Heading]]` を shell / leaf へ解決する
- part 分割された見出しは section shell へ解決する
- backlinks を entry / shell / leaf 解像度で付ける
- `_unresolved.md` に source page と source line range を出す

### 機械完了条件

```sh
cargo test link_resolution_agentic
cargo test acceptance::wikilinks_resolve_to_shell_or_leaf_for_large_notes
cargo test acceptance::unresolved_links_include_source_location
```

Gate 条件:

- `checks[id=agentic-link-graph].passed == true`
- `summary.broken_local_links == 0`
- `summary.unresolved_links == expected_unresolved_links`

## 21.14 AG-10 hard limit enforcement

### 作業

- renderer 書き込み後の warn ではなく、planning / gate で 40,000 文字超を failure にする
- どうしても分割できない入力は、理由付き planning error にする
- v1.2 互換 mode が必要な場合は option として分離する

### 機械完了条件

```sh
cargo test page_budget
cargo test acceptance::agentic_output_never_exceeds_hard_limit
```

Gate 条件:

- `checks[id=agentic-page-budget].passed == true`
- report の oversized page list が空

Negative test:

- 意図的に 40,001 文字の synthetic page plan を作る unit test が error を返す

## 21.15 AG-11 simulated agent query checks

### 作業

- gate 内に模擬 agent 探索 runner を実装する
- file list / read file / search text / follow link のみを使う
- 20.12 の query fixture をすべて実行する
- 読んだ page 数、読んだ総文字数、到達 page、traceability を report に記録する

### 機械完了条件

```sh
cargo test agentic_search_gate::tests::simulated_agent_reaches_expected_pages
```

Gate 条件:

- `checks[id=agentic-query-fixtures].passed == true`
- `checks[id=agentic-no-full-read].passed == true`
- `checks[id=agentic-traceability].passed == true`

Metrics 条件:

- query ごとに `read_pages <= expected_max_read_pages`
- query ごとに `read_chars <= expected_max_read_chars`
- exact phrase query で source 全体を読んでいない
- huge single line query で byte range が返る

## 21.16 AG-12 `scripts/verify.sh` / CI 統合

### 作業

- `scripts/verify.sh` に normal agentic gate を追加する
- `.github/workflows/verify.yml` が `scripts/verify.sh` を通じて agentic gate を実行する状態にする
- report path を deterministic にする

### 機械完了条件

```sh
scripts/verify.sh
```

完了条件:

- `scripts/verify.sh` が agentic gate を実行する
- `target/agentic-search-gate/report.json` が生成される
- report の `passed == true`
- report の `score == max_score`

## 21.17 AG-13 heavy gate

### 作業

- heavy mode fixtures を追加する
- 20 MiB と 200 MiB の resource budget を検査する
- many-headings / many-links で paging 発生を検査する
- peak RSS を取得できる環境では report に記録する

### 機械完了条件

```sh
cargo run --quiet --example agentic_search_gate -- \
  --mode heavy \
  --work-dir target/agentic-search-heavy \
  --report target/agentic-search-heavy/report.json \
  --min-score 100 \
  --require-resource-budget
```

Report 条件:

- `passed == true`
- `score == max_score`
- `summary.oversized_pages == 0`
- `summary.elapsed_ms <= 300000`
- `checks[id=agentic-page-budget].passed == true`
- `checks[id=agentic-determinism].passed == true`

## 21.18 AG-14 docs / README 更新

### 作業

- README に `agent/` 出力、agent guide、catalog、term index、gate command を追記する
- 13 / 17 / 18 / 19 / 20 章と矛盾がないか更新する
- v1.2 soft limit と vNext hard limit の違いを明記する

### 機械完了条件

```sh
cargo test docs
cargo run --quiet -- init docs/要件定義 -o target/docs-wiki-smoke
```

生成物条件:

- `target/docs-wiki-smoke/index.md` が存在する
- `target/docs-wiki-smoke/fragments/_index.md` が存在する
- `target/docs-wiki-smoke/headings/index.md` が存在する
- 既存 docs test が通る

## 21.19 進捗判定マトリクス

| Milestone | 含むタスク | 完了判定 |
|---|---|---|
| M1 Gate skeleton | AG-00, AG-01 | `agentic_search_gate --min-score 0` が report を出し、全 check ID が存在 |
| M2 Planning foundation | AG-02, AG-03, AG-04 | `cargo test input_classifier regular_page_plan page_registry` が通る |
| M3 Metadata and indexes | AG-05, AG-06, AG-07 | `agentic-metadata`, `agentic-catalog`, `agentic-term-index`, `agentic-navigation` が pass |
| M4 CLI integration | AG-08, AG-09, AG-10 | 2〜5 MiB fixture の通常 CLI 生成で oversized page 0 |
| M5 Agentic validation | AG-11, AG-12 | `scripts/verify.sh` が agentic gate を含んで pass |
| M6 Heavy readiness | AG-13, AG-14 | heavy report が満点、README/docs smoke が pass |

## 21.20 実装時の停止条件

以下のいずれかが発生した場合は、そのタスクを完了扱いにしない。

- `cargo test` が失敗する
- `cargo clippy -- -D warnings` が失敗する
- 生成 `.md` に 40,000 文字超が 1 件でもある
- `agentic_search_gate` report の `passed` が `false`
- broken local links が 1 件でもある
- source file hash が実行前後で変わる
- 同一 fixture の 2 回生成が byte-identical でない
- agent query fixture が source 全体または全 leaf の読み込みを必要とする

この停止条件に該当した場合は、該当 check の failure を先に固定し、実装を追加してから再度同じ gate で通す。
