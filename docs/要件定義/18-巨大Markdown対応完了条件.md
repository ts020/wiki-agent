# 18. 巨大 Markdown 対応 完了条件

本章は、2 MiB を超える巨大 Markdown を agentic search に適した構造・サイズへ変換できたと言えるための完了条件を定義する。

現行 v1.2 は 1 ページ 40,000 文字以内を目安にした断片化を持つが、入力ファイル自体は走査ガードや全読み込み前提の影響を受ける。本章はその次段階として、入力サイズに依存せず、出力ページを常に小さく保つための受け入れ条件を固定する。

実装設計と機械検証方法は [19. 巨大 Markdown 対応 設計と機械検証](19-巨大Markdown対応設計と機械検証.md) で定義する。

## 18.1 ゴール

巨大 Markdown 対応のゴールは、巨大な `.md` を「読む」のではなく、**検索・探索しやすい小さな Markdown ページ群へコンパイルすること**である。

完了状態では、以下を満たす。

- 入力 `.md` のファイルサイズが大きくても処理できる
- 生成される個々のページは agent が 1 回で読める上限内に収まる
- root / 階層 index / section shell / leaf page を辿れば、必要箇所へ段階的に到達できる
- 入力本文は非破壊で、生成物は純粋な Markdown のまま残る
- 外部 API、AI、ネットワークに依存しない
- 同一入力から同一出力を再現できる

## 18.2 用語

| 用語 | 意味 |
|---|---|
| 巨大 Markdown | 2 MiB 以上の UTF-8 `.md` ファイル |
| 超巨大 Markdown | 200 MiB 以上の UTF-8 `.md` ファイル |
| section tree | Markdown の見出し階層、本文範囲、行数、byte range を持つ構造メタデータ |
| shell page | 大きな section を代表し、子ページ一覧とナビだけを持つページ |
| leaf page | 実本文を持つ最小読み込み単位のページ |
| page budget | 生成ページ 1 枚に許される文字数・行数・byte range の上限 |
| forced split | 見出し等の自然境界が無い場合に、段落・行・byte window で強制分割すること |
| agentic search | agent が root から階層 index と隣接リンクを辿り、必要な leaf だけを読む探索方法 |

## 18.3 入力サイズの完了条件

巨大 Markdown 対応完了時点では、入力サイズについて以下を満たす。

| ケース | 完了条件 |
|---|---|
| 2 MiB `.md` | 警告なし、または情報ログのみで取り込み、生成完了する |
| 20 MiB `.md` | 生成完了し、全 leaf page が page budget 内に収まる |
| 200 MiB `.md` | 生成完了する。処理時間・メモリは非機能条件を満たす |
| 1 GiB `.md` | 必須完了条件ではない。ただし設計上、入力全体の `String` 化を前提にしない |

ファイルサイズによる一律スキップは、少なくとも 200 MiB までは行わない。スキップが必要な場合は、サイズそのものではなく以下の理由に限定する。

- UTF-8 として解釈できない
- NULL byte 等によりバイナリと判定される
- OS / 権限 / I/O エラーで読み込めない
- 明示された管理上限を超え、かつユーザが opt-in していない

## 18.4 処理方式の完了条件

実装は以下の性質を持つこと。

- 入力ファイル全体を 1 つの `String` として保持しない
- 行単位、byte range 単位、または bounded buffer で走査できる
- frontmatter、見出し、コードフェンス、HTML コメント、wikilink、通常 Markdown link を streaming に近い形で検出できる
- first pass で section tree と link metadata を作り、second pass で必要な byte range だけを読み直して leaf page を出力できる
- 処理中に一時ファイルを使う場合は、出力先とは別の作業領域に置き、異常終了時も入力を変更しない
- 同一入力・同一設定では byte-identical な出力になる

## 18.5 分割アルゴリズムの完了条件

分割は固定の h2 / h3 だけに依存しない。各 section に対して、以下の優先順位で page budget 内へ収める。

1. 子見出しで分割する
2. 見出しが不足する場合は段落境界で分割する
3. 段落が巨大な場合はリスト項目、表行、コードブロック境界で分割する
4. それでも巨大な場合は行 window で `part-001.md`, `part-002.md`, ... に分割する
5. 単一行が page budget を超える場合は byte window で分割する

完了時点では以下を満たす。

- h1〜h6 の見出し階層を section tree として保持する
- 既存 v1 の h2/h3 分割規則は、巨大 Markdown でない通常入力でも回帰しない
- h2 が無い巨大 Markdown でも leaf page へ分割される
- h3 以降が無い巨大 h2 でも leaf page へ分割される
- 巨大なコードブロックは、可能な限りフェンスを壊さずに分割する
- フェンスを壊さざるを得ない場合は、各 leaf が Markdown として読めるように継続フェンスを明示する
- 巨大な表は行単位で分割し、各 leaf に必要な header row を再掲する
- 改行を含まない単一巨大段落／単一巨大行は byte window で `part-001.md`, `part-002.md`, ... に分割し、全 leaf を hard limit 内に収める

## 18.6 出力構造の完了条件

出力は既存の `fragments/` 構造を保ちつつ、任意深度の section / part を表現できること。

例:

```text
md-wiki/
├── index.md
├── fragments/
│   ├── _index.md
│   └── huge-note/
│       ├── index.md
│       ├── chapter-1/
│       │   ├── index.md
│       │   ├── topic-a.md
│       │   └── topic-b/
│       │       ├── index.md
│       │       ├── part-001.md
│       │       └── part-002.md
│       └── no-heading-content/
│           ├── index.md
│           ├── part-001.md
│           └── part-002.md
├── headings/
├── links/
├── tags/
└── _unresolved.md
```

完了時点では以下を満たす。

- root `index.md` は巨大入力でも全 leaf を列挙しない
- `fragments/_index.md` と各階層 `_index.md` は page budget 内に収まる
- `_index.md` が肥大化する場合は `_index-001.md`, `_index-002.md`, ... のようにページングされる
- shell page は子ページ一覧、summary metadata、Parent / Prev / Next 相当の導線を持つ
- leaf page は本文、Parent、Prev、Next、Backlinks を持つ
- leaf page から source file と byte range を追跡できる
- 出力パスは安定し、同一見出し衝突時は既存の `-1`, `-2` 規則と整合する

## 18.7 ページサイズの完了条件

生成ページは以下の budget を満たす。

| 種別 | target | hard limit |
|---|---:|---:|
| leaf page | 20,000〜30,000 文字 | 40,000 文字 |
| shell page | 10,000〜20,000 文字 | 40,000 文字 |
| `_index.md` / paged index | 10,000〜20,000 文字 | 40,000 文字 |
| `headings/` / `links/` / `tags/` index | 10,000〜20,000 文字 | 40,000 文字 |
| `_unresolved.md` | 10,000〜20,000 文字 | 40,000 文字 |

hard limit を超えるページが生成された場合、巨大 Markdown 対応は未完了とみなす。警告ログだけで済ませてよいのは v1.2 までであり、本章の完了条件では **hard limit 超過 0 件** を要求する。

## 18.8 Agentic Search Metadata

各 shell / leaf page は、agent が探索判断に使える metadata を持つこと。形式は YAML frontmatter または HTML comment のいずれかでよいが、機械的に parse できる必要がある。

ツール使用型エージェントがこの metadata と各索引をどう辿るか、また agent guide / page catalog / term index を含む探索契約は [20. ツール使用型エージェント向け探索仕様と設計](20-ツール使用型エージェント向け探索仕様と設計.md) で定義する。

最低限の metadata:

```yaml
---
md_wiki:
  source: huge.md
  page_kind: leaf
  section_path: ["Chapter 2", "Topic A"]
  heading_level: 3
  byte_range: [123456, 145678]
  line_range: [3400, 3720]
  parent: index.md
  prev: topic-a-001.md
  next: topic-a-003.md
---
```

完了時点では以下を満たす。

- source file を特定できる
- 元ファイル内 byte range と line range を特定できる
- section path を配列として取得できる
- shell / leaf / paged index の種別を取得できる
- Parent / Prev / Next が metadata と本文ナビの双方で一致する
- metadata を除いても Markdown として読める

## 18.9 リンク・索引の完了条件

巨大 Markdown でも既存の wikilink / tag / heading / link / backlink 要件を維持する。

- `[[Note]]` は対象 note の入口または最適な shell へ解決される
- `[[Note#Heading]]` は該当 section の shell / leaf へ解決される
- 見出しが part 分割された場合、`#Heading` は section shell を指す
- leaf 内の通常 Markdown link は原文保持する
- Backlinks は leaf / shell 解像度で付く
- `headings/` は巨大入力でも 40,000 文字を超えないようページングされる
- `links/` は巨大入力でも 40,000 文字を超えないようページングされる
- `_unresolved.md` は巨大入力でも 40,000 文字を超えないようページングされる

## 18.10 非機能完了条件

巨大 Markdown 対応は、以下の非機能条件を満たす。

| 項目 | 完了条件 |
|---|---|
| メモリ | peak RSS が `min(入力サイズ * 0.25, 512 MiB) + 固定オーバーヘッド` を目安に収まる。少なくとも 200 MiB 入力で入力全体の複製を保持しない |
| 処理時間 | 20 MiB 入力を通常開発機で 30 秒以内、200 MiB 入力を 5 分以内に処理する |
| 再現性 | 同一入力・同一設定で byte-identical |
| 非破壊 | 入力 `.md` は変更しない |
| オフライン | 外部 API / ネットワークを使わない |
| 中断耐性 | 異常終了時に入力を壊さず、次回実行で出力を再生成できる |
| ログ | skip / warning / split fallback / hard limit failure を構造化ログで追える |

## 18.11 検証データセット

完了判定には、最低限以下の fixture を用意する。

| Fixture | サイズ | 内容 | 期待 |
|---|---:|---|---|
| `large-h2-h3.md` | 2〜5 MiB | h2/h3 が豊富な構造化文書 | h3 leaf へ分割、全ページ hard limit 内 |
| `large-no-heading.md` | 2〜5 MiB | 見出し無しの段落集合 | `part-001.md` 形式で分割 |
| `large-single-heading.md` | 2〜5 MiB | h1 + 巨大本文のみ | 段落 / 行 window で分割 |
| `large-single-line.md` | 3 MiB 以上 | 改行を含まない単一巨大段落 | byte window で分割、全ページ hard limit 内 |
| `large-code-block.md` | 2〜5 MiB | 巨大 fenced code block | Markdown として読める leaf へ分割 |
| `large-table.md` | 2〜5 MiB | 巨大 Markdown table | header row を維持して分割 |
| `huge-20mb.md` | 20 MiB | 混合構造 | 全ページ hard limit 内、30 秒以内 |
| `huge-200mb.md` | 200 MiB | 混合構造 | 入力全体を保持せず処理、5 分以内 |
| `many-headings.md` | 任意 | 見出し 50,000 件以上 | `headings/` がページングされる |
| `many-links.md` | 任意 | 通常 link / wikilink 50,000 件以上 | `links/` / `_unresolved` がページングされる |

巨大 fixture はリポジトリに実体をコミットしない。テスト時に deterministic に生成するか、`.context/` / `target/` 配下に生成する。

## 18.12 自動検証の完了条件

以下の検証が `scripts/verify.sh` または専用の heavy gate で実行可能であること。

### 通常 Gate

通常 Gate は 2〜5 MiB の fixture を使い、PR ごとに実行する。

- `large-h2-h3.md`
- `large-no-heading.md`
- `large-single-heading.md`
- `large-single-line.md`
- `large-code-block.md`
- `large-table.md`

合格条件:

- すべて取り込まれる
- hard limit 超過ページ 0
- root / `_index.md` / headings / links / tags / unresolved の hard limit 超過 0
- `large-single-line.md` 由来の leaf は `split_reason: byte_window` を持ち、全 leaf が hard limit 内に収まる
- leaf page に Parent がある
- 連続 leaf に Prev / Next がある
- source byte range / line range metadata がある
- 同一入力 2 回生成で byte-identical

### Heavy Gate

Heavy Gate は毎回の PR 必須ではなく、main branch、nightly、または明示実行でよい。

- `huge-20mb.md`
- `huge-200mb.md`
- `many-headings.md`
- `many-links.md`

合格条件:

- 処理時間基準を満たす
- メモリ基準を満たす
- hard limit 超過ページ 0
- index paging が発生し、全 paged index が hard limit 内
- 生成物の総リンクが壊れていない

## 18.13 品質スコアへの追加条件

巨大 Markdown 対応後、品質スコアに以下の check を追加する。

| Check ID | 配点 | 判定基準 |
|---|---:|---|
| `large-md-ingestion` | 10 | 2 MiB 超の UTF-8 `.md` がスキップされず取り込まれる |
| `large-md-page-budget` | 20 | 巨大入力由来の全生成ページが 40,000 文字以内 |
| `large-md-navigation` | 10 | shell / leaf の Parent / Prev / Next metadata と本文ナビが一致する |
| `large-md-forced-split` | 15 | 見出し無し・単一巨大 section・巨大コードブロック・巨大表が leaf へ分割される |
| `large-md-index-paging` | 15 | headings / links / unresolved / `_index` が 40,000 文字以内にページングされる |
| `large-md-determinism` | 10 | 巨大 fixture の再生成が byte-identical |
| `large-md-resource-budget` | 20 | 20 MiB fixture が処理時間・メモリ基準内 |

通常品質スコアとは別に `heavy_quality_score` を持ってよい。通常 PR gate は軽量 fixture、main/nightly は heavy fixture を対象にする。

## 18.14 完了判定

巨大 Markdown 対応は、以下をすべて満たした時点で完了とする。

- 18.3 の入力サイズ条件を満たす
- 18.4 の処理方式条件を満たす
- 18.5 の再帰的分割条件を満たす
- 18.6 の出力構造条件を満たす
- 18.7 の page budget hard limit 超過 0 を満たす
- 18.8 の metadata 条件を満たす
- 18.9 のリンク・索引条件を満たす
- 18.10 の非機能条件を満たす
- 18.11 の fixture が用意されている
- 18.12 の通常 Gate と Heavy Gate が実行可能で、合格する
- 18.13 の品質スコア check が実装され、履歴に記録される

いずれかが未達の場合、「巨大 Markdown を agentic search に適した構造・サイズへ分割できる」とは判定しない。
