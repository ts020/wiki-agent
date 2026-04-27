# 19. 巨大 Markdown 対応 設計と機械検証

本章は [18. 巨大 Markdown 対応 完了条件](18-巨大Markdown対応完了条件.md) を実装設計と機械検証方法へ落とすための仕様である。

18 章は「何を満たせば完了か」を定義する。本章は「どの構造で実装し、どのコマンドとレポートで完了を判定するか」を定義する。

## 19.1 設計方針

巨大 Markdown 対応は、既存の h2 / h3 断片化を置き換えるのではなく、通常サイズ入力では既存挙動を維持し、巨大入力でだけ任意深度の section / part 生成へ拡張する。

実装は以下の方針を守る。

- 入力 Markdown を全文 `String` として保持しない
- 先に出力計画を確定し、その計画に従って出力する
- 出力計画は同一入力・同一設定で byte-identical になる
- 生成ページの hard limit は警告ではなく gate failure として扱う
- 検証 fixture は deterministic に生成し、リポジトリに巨大ファイル実体をコミットしない
- 検証結果は JSON report を唯一の機械判定入力にする

## 19.2 コンポーネント設計

巨大 Markdown 対応は以下のコンポーネントに分ける。

| コンポーネント | 責務 | 主な出力 |
|---|---|---|
| `LargeInputScanner` | UTF-8 / NULL / binary 判定、行単位走査、byte offset と line number の採番 | `LineRecord` stream |
| `MarkdownEventScanner` | frontmatter、見出し h1〜h6、fence、HTML comment、wikilink、Markdown link、表境界を検出 | `MarkdownEvent` stream |
| `SectionTreeBuilder` | 見出し階層と本文範囲を section tree として構築 | `SectionTree` |
| `PagePlanner` | section tree と page budget から shell / leaf / paged index の出力計画を作る | `PagePlan` |
| `RangeRenderer` | `PagePlan` の byte range だけを再読込し、本文と metadata を出力する | Markdown pages |
| `IndexPager` | `_index.md`、`headings/`、`links/`、`tags/`、`_unresolved.md` を 40,000 文字以内にページングする | paged index pages |
| `LargeMdGate` | fixture 生成、CLI 実行、出力検査、resource 計測、JSON report 生成 | `LargeMdGateReport` |

通常サイズの入力は既存の `scan`、`ingest_notes`、`build_nodes`、`write_wiki` の経路を維持してよい。ただし巨大入力と通常入力が同一ディレクトリに混在する場合、最終的な relations / indexes は両経路の `PagePlan` を統合してから生成する。

## 19.3 データモデル

### `LineRecord`

`LargeInputScanner` は入力行ごとに以下を持つ。

| フィールド | 型 | 条件 |
|---|---|---|
| `byte_start` | `u64` | UTF-8 byte offset。前行の `byte_end` 以上 |
| `byte_end` | `u64` | 改行を含む場合は改行 byte を含む |
| `line_number` | `u64` | 1 始まり |
| `char_count` | `u32` | 行の Unicode scalar count |
| `kind_hint` | enum | `plain` / `blank` / `heading` / `fence` / `table` / `list` |

### `SectionNode`

`SectionTreeBuilder` は見出しと本文範囲を以下で表す。

| フィールド | 型 | 条件 |
|---|---|---|
| `id` | stable string | source path + heading path + occurrence から決定 |
| `parent_id` | optional string | root section はなし |
| `level` | `u8` | root は 0、Markdown heading は 1〜6 |
| `title` | string | 見出しなし root / forced section は生成タイトル |
| `slug` | string | 既存 slug 規則と衝突 suffix に従う |
| `byte_range` | `[u64, u64]` | source 内の範囲 |
| `line_range` | `[u64, u64]` | source 内の範囲 |
| `children` | ordered list | source 出現順 |
| `links` | ordered list | source 出現順の link metadata |

### `PagePlan`

`PagePlanner` はレンダリング前に全ページを確定する。

| フィールド | 型 | 条件 |
|---|---|---|
| `page_id` | stable string | 2 回実行で同一 |
| `page_kind` | enum | `entry` / `shell` / `leaf` / `paged_index` |
| `output_path` | path | 出力 root からの相対 path |
| `source_path` | optional path | source 由来ページのみ必須 |
| `section_path` | list string | shell / leaf は必須 |
| `byte_ranges` | list `[u64, u64]` | leaf は 1 個以上。synthetic text は含めない |
| `line_ranges` | list `[u64, u64]` | leaf は 1 個以上 |
| `split_reason` | enum | `heading` / `paragraph` / `list` / `table` / `code_fence` / `line_window` / `byte_window` |
| `parent` | optional path | shell / leaf / paged_index は必須 |
| `prev` | optional path | 連続ページの先頭以外は必須 |
| `next` | optional path | 連続ページの末尾以外は必須 |
| `estimated_chars` | `usize` | 40,000 以下でなければ planning failure |

## 19.4 分割設計

分割は以下の順で行う。

1. source ごとに root section を作る
2. h1〜h6 を section tree に入れる
3. `estimated_chars <= 30,000` の section は leaf 候補にする
4. `estimated_chars > 30,000` の section は子見出しへ展開する
5. 子見出しで収まらない section は段落境界へ展開する
6. 段落境界で収まらない section は list item / table row / code fence 境界へ展開する
7. それでも収まらない section は line window へ展開する
8. 単一行が 40,000 文字を超える場合のみ byte window にする

shell page は「子 leaf または子 shell への導線だけを持つページ」とする。shell page に source 本文を複製しない。

leaf page は本文を持つ最小読み込み単位とする。leaf page の本文が 40,000 文字を超える `PagePlan` は作成してはならない。

byte window は UTF-8 の文字境界で分割する。1 つの leaf が 1 行の一部だけを持つ場合でも、`byte_ranges` は source 内の実 byte range を示し、`line_ranges` は同じ source line を指してよい。

## 19.5 出力 metadata

shell / leaf / paged index は、先頭に YAML frontmatter と本文ナビを持つ。

```yaml
---
md_wiki:
  schema_version: 1
  page_kind: leaf
  source: huge.md
  section_path: ["Chapter 2", "Topic A"]
  heading_level: 3
  byte_ranges:
    - [123456, 145678]
  line_ranges:
    - [3400, 3720]
  split_reason: paragraph
  parent: index.md
  prev: topic-a-001.md
  next: topic-a-003.md
---
```

本文ナビは frontmatter の直後に置く。

```md
> Parent: [Chapter 2](index.md) · Prev: [Topic A 1](topic-a-001.md) · Next: [Topic A 3](topic-a-003.md)
---
```

機械検証では、metadata の `parent` / `prev` / `next` と本文ナビのリンク先が一致しない場合は failure とする。

## 19.6 機械検証コマンド

巨大 Markdown 対応の検証は、通常 gate と heavy gate に分ける。

通常 gate:

```sh
cargo run --quiet --example large_md_gate -- \
  --mode normal \
  --work-dir target/large-md-gate \
  --report target/large-md-gate/report.json \
  --min-score 100
```

heavy gate:

```sh
cargo run --quiet --example large_md_gate -- \
  --mode heavy \
  --work-dir target/large-md-heavy \
  --report target/large-md-heavy/report.json \
  --min-score 100 \
  --require-resource-budget
```

`scripts/verify.sh` は、巨大 Markdown 実装が通常 gate を通る段階になったら normal mode を呼び出す。heavy mode は main branch、nightly、または明示実行用の別 CI job とする。

## 19.7 JSON report contract

`large_md_gate` は以下の JSON を stdout と `--report` の両方に出す。

```json
{
  "schema_version": 1,
  "mode": "normal",
  "commit": "abcdef123456",
  "passed": true,
  "score": 100.0,
  "max_score": 100.0,
  "summary": {
    "fixtures": 5,
    "input_bytes": 12582912,
    "generated_pages": 340,
    "max_page_chars": 28120,
    "oversized_pages": 0,
    "broken_local_links": 0,
    "unresolved_links": 0,
    "input_hash_changed": false,
    "byte_identical_rerun": true,
    "peak_rss_bytes": 134217728,
    "elapsed_ms": 8200
  },
  "checks": [
    {
      "id": "large-md-page-budget",
      "passed": true,
      "points": 20.0,
      "max_points": 20.0,
      "metrics": {
        "hard_limit": 40000,
        "oversized_pages": 0,
        "max_page_chars": 28120
      }
    }
  ]
}
```

CI は `passed == true` と `score == max_score` の両方を要求する。個別 check の詳細は Markdown summary ではなく JSON の `checks[].metrics` を正とする。

## 19.8 Check ID と判定式

| Check ID | 対象 | 判定式 |
|---|---|---|
| `large-md-ingestion` | 18.3 | 全 fixture で `skipped_due_to_size == 0`、かつ source ごとの entry または shell が生成される |
| `large-md-streaming` | 18.4 | `max_buffered_bytes <= 8 MiB`、かつ 20 MiB 以上 fixture で `input_full_string_reads == 0` |
| `large-md-section-tree` | 18.4 / 18.5 | h1〜h6 の出現数と `SectionNode` 数が一致し、親子 level が単調に整合する |
| `large-md-forced-split` | 18.5 | no-heading、single-heading、single-line、code-block、table fixture の各 leaf が 40,000 文字以内で、`split_reason` が期待種別を含む。single-line は `byte_window` を必須とする |
| `large-md-page-budget` | 18.7 | 生成された `.md` の `chars().count() <= 40,000` が全ページで真 |
| `large-md-metadata` | 18.8 | shell / leaf / paged_index の metadata が parse でき、source、page_kind、range、navigation が必須条件を満たす |
| `large-md-navigation` | 18.6 / 18.8 | parent / prev / next のリンク先が存在し、連続 leaf の `next.prev == self` が成り立つ |
| `large-md-range-coverage` | 18.4 / 18.8 | source ごとの leaf `byte_ranges` が重複せず、出力対象本文範囲を欠落なく覆う |
| `large-md-index-paging` | 18.6 / 18.9 | `_index`、`headings`、`links`、`tags`、`_unresolved` の全ページが 40,000 文字以内で、paging manifest のリンクが存在する |
| `large-md-link-integrity` | 18.9 | 生成 Markdown の相対リンクが全て存在し、未解決 wikilink 件数が expected unresolved count と一致する |
| `large-md-determinism` | 18.10 | 同一 fixture を 2 回生成した output tree の file path と bytes が完全一致する |
| `large-md-non-destructive` | 18.10 | fixture 入力の file hash が実行前後で一致する |
| `large-md-resource-budget` | 18.10 | normal は 2〜5 MiB fixture 全体が 60 秒以内、heavy は 20 MiB が 30 秒以内、200 MiB が 5 分以内、RSS が基準以内 |

## 19.9 Fixture 生成設計

fixture は `target/large-md-gate/fixture/` または `target/large-md-heavy/fixture/` に毎回生成する。

| Fixture | normal | heavy | 検証目的 |
|---|---:|---:|---|
| `large-h2-h3.md` | 2〜5 MiB | 20 MiB | heading split |
| `large-no-heading.md` | 2〜5 MiB | 20 MiB | forced paragraph / line split |
| `large-single-heading.md` | 2〜5 MiB | 20 MiB | single section split |
| `large-single-line.md` | 3 MiB | 20 MiB | single paragraph byte window split |
| `large-code-block.md` | 2〜5 MiB | 20 MiB | code fence continuation |
| `large-table.md` | 2〜5 MiB | 20 MiB | table row split and header repeat |
| `huge-200mb.md` | なし | 200 MiB | memory and elapsed time |
| `many-headings.md` | 50,000 headings | 200,000 headings | headings paging |
| `many-links.md` | 50,000 links | 200,000 links | links / unresolved paging |

生成器は固定 seed を使う。fixture 本文には連番 marker を入れ、`large-md-range-coverage` が欠落と重複を検出できるようにする。

## 19.10 合否判定

通常 gate は以下をすべて満たした場合のみ合格とする。

- `large_md_gate --mode normal` が exit code 0
- JSON report の `passed` が `true`
- `score == max_score`
- `large-md-resource-budget` 以外の check がすべて pass
- normal resource budget が pass

heavy gate は以下をすべて満たした場合のみ合格とする。

- `large_md_gate --mode heavy --require-resource-budget` が exit code 0
- JSON report の `passed` が `true`
- `score == max_score`
- `large-md-resource-budget` が 20 MiB と 200 MiB の両方で pass
- `large-md-index-paging` が many-headings と many-links の両方で paging 発生を確認している

この判定を満たさない場合、18 章の巨大 Markdown 対応は未完了とする。

## 19.11 実装順序

実装は以下の順で進める。

1. `large_md_gate` の fixture generator と JSON report skeleton を作る
2. 現行実装に対する failing checks を固定する
3. `LargeInputScanner` と `MarkdownEventScanner` を実装する
4. `SectionTreeBuilder` と `PagePlanner` を実装する
5. `RangeRenderer` と metadata 出力を実装する
6. `IndexPager` を導入し、巨大 indexes を 40,000 文字以内へ分割する
7. normal gate を `scripts/verify.sh` に追加する
8. heavy gate を CI の明示実行 job として追加する

各段階は、既存 AC-01〜AC-23 の `cargo test` と品質スコアを落とさないことを前提にする。
