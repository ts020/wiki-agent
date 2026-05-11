# md-wiki OSS 公開作業計画

作成日: 2026-05-07

本書は `md-wiki` を OSS として公開し、`v0.1.0` 初回リリースへ到達するための作業計画である。対象は公開準備、配布準備、ドキュメント、CI、リリース運用であり、Markdown wiki 生成動作そのものの機能追加は扱わない。

## ゴール

- 第三者がライセンス条件を理解した上で利用、再配布、改変できる
- GitHub リポジトリを公開しても内部作業ファイルやローカル文脈が配布物に混入しない
- README だけでインストール、最小実行、出力確認、既知の制限を把握できる
- `cargo package --locked` と既存の継続検証 gate が通る
- `v0.1.0` の GitHub Release と、必要に応じて crates.io publish を実行できる

## 非ゴール

- 差分更新、watch mode、HTML 出力、全文検索 index などの将来拡張を実装する
- 既存の Markdown 生成仕様を変更する
- OSS 公開と無関係な内部 refactor を行う
- 外部 API、AI、embedding、ネットワーク依存を導入する

## 現状の公開ブロッカー

| 優先度 | 項目 | 状態 | 対応方針 |
|---|---|---|---|
| P0 | ライセンス | `LICENSE` が存在せず、`Cargo.toml` にも license 指定がない | `MIT` または `MIT OR Apache-2.0` を決めて明記する |
| P0 | Cargo package metadata | `description`, `repository`, `homepage` / `documentation`, `keywords`, `categories`, `rust-version` が不足 | `Cargo.toml` に公開用 metadata を追加する |
| P0 | package 混入物 | `.context/`, `.agents/`, `.claude/`, `Cargo.toml.orig`, 作業履歴が package に入り得る | `package.exclude` または `package.include` で配布対象を固定する |
| P1 | 要件定義の古い記述 | 1 MiB 超 Markdown の扱いが README / 実装 / 要件で揺れている | large path 対応済みの現行挙動に合わせて要件文を更新する |
| P1 | README | 初見利用者向けの install、最小例、制限事項、troubleshooting が薄い | README を利用者導線中心に再構成する |
| P1 | OSS 運用文書 | `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md` がない | 最小でよいので初回公開用に追加する |
| P2 | CI | Ubuntu のみ。package verification、locked test、OS matrix がない | 既存 gate を維持しつつ公開前検証 job を追加する |
| P2 | 品質履歴 | `latest.md` が dirty な古い記録を指している | 公開前コミットで再記録する |

## Phase 1: 公開阻害要因の解消

### 作業

1. ライセンスを決定して `LICENSE` を追加する。
2. `Cargo.toml` に以下を追加する。
   - `description`
   - `license` または `license-file`
   - `repository`
   - `homepage` または `documentation`
   - `keywords`
   - `categories`
   - `rust-version`
3. package 対象を明示する。
   - 内部 agent 用ファイルを crates.io package に含める必要があるか判断する
   - 不要な場合は `package.exclude` で `.context/`, `.agents/`, `.claude/`, `Cargo.toml.orig`, ローカル履歴を除外する
4. `.gitignore` に `.context/` を追加する。
5. `cargo package --locked` を通す。

### 完了条件

- `cargo package --locked` が成功する
- `cargo package --no-verify --list` に不要な内部作業ファイルが含まれない
- ライセンス条件が GitHub と crates.io の両方で読める

## Phase 2: 要件定義の整合性修正

### 作業

1. `docs/要件定義/07-入力.md` の走査ガードを現行の large Markdown 対応に合わせる。
2. `docs/要件定義/12-エラー処理.md` の 1 MiB 超 Markdown の旧スキップ記述を更新する。
3. `docs/要件定義/15-確定済み仕様.md` の走査上限表を更新する。
4. v1.2 と vNext / agentic search の境界を、現在の実装状態と矛盾しないよう明記する。
5. README と docs の内容整合を検査するテストが不足していれば追加する。

### 完了条件

- README、要件定義、実装が 1 MiB 超 `.md` の扱いで矛盾しない
- `cargo test --test docs` が成功する
- `scripts/verify.sh` が成功する

## Phase 3: 利用者向け README 整備

### 作業

1. インストール方法を追加する。
   - `cargo install --path .`
   - crates.io 公開後の `cargo install md-wiki-cli`（binary 名は `md-wiki`）
   - GitHub Releases で binary 配布する場合の導線
2. 最小サンプルを追加する。
   - 入力 Markdown 例
   - 実行コマンド
   - 生成される主要ファイル
3. 主要仕様を利用者目線で整理する。
   - `.md` 専用
   - `wiki: false`
   - `fragment: false`
   - `[[wikilink]]`
   - tags / headings / links / backlinks / unresolved
   - agent guide / page catalog / term index
4. 安全性と制限事項を明記する。
   - 入力非破壊
   - 出力先 cleanup の保護条件
   - 外部 API / ネットワーク非依存
   - 未対応構文
5. 類似 OSS との差別化を短く追記する。
   - 生成物も Markdown
   - agentic search 向けの小ページ / metadata / index
   - AI / API 非依存

### 完了条件

- 初見利用者が README だけで 5 分以内に最小実行できる
- 失敗しやすい出力先 cleanup と入力対象の制限が README で分かる
- README と docs の整合性テストが成功する

## Phase 4: OSS 運用ファイル追加

### 作業

1. `CONTRIBUTING.md` を追加する。
   - 要件定義を正とすること
   - 開発コマンド
   - PR 前に実行する gate
   - 仕様変更時の docs 更新ルール
2. `SECURITY.md` を追加する。
   - 脆弱性報告方法
   - サポート対象 version
   - 公開 issue に書かない情報
3. `CHANGELOG.md` を追加する。
   - `v0.1.0` の初回公開内容
   - 未リリース欄
4. GitHub issue / PR template を追加する。
   - bug report
   - feature request
   - requirements / spec change
   - PR checklist
5. 必要であれば `CODE_OF_CONDUCT.md` を追加する。

### 完了条件

- 外部 contributor が PR 作成前に読むべき文書が揃っている
- Issue / PR で要件定義、テスト、公開可否の確認漏れが起きにくい

## Phase 5: CI とリリース検証強化

### 作業

1. 既存の `.github/workflows/verify.yml` に公開前検証を追加する。
   - `cargo test --locked`
   - `cargo package --locked`
2. OS matrix を追加する。
   - `ubuntu-latest`
   - `macos-latest`
   - `windows-latest`
3. dependency audit を検討する。
   - 最初は `cargo audit` または `cargo deny` を別 job にする
   - 導入によるメンテナンス負荷が高ければ Phase 6 後へ送る
4. 品質履歴を公開前コミットで再記録する。
   - `scripts/record-quality-score.sh`
5. `scripts/verify.sh` の所要時間を確認し、PR 必須 gate と nightly / manual gate の分離が必要か判断する。

### 完了条件

- GitHub Actions が主要 OS で成功する
- `cargo package --locked` が CI で検証される
- 品質履歴が公開直前の clean commit を指す

## Phase 6: 公開

### 作業

1. GitHub 公開前チェックを行う。
   - README
   - LICENSE
   - Cargo metadata
   - CI status
   - package contents
   - 不要な内部ファイル混入
2. `v0.1.0` の release commit を作る。
   - `Cargo.toml` version
   - `Cargo.lock`
   - `CHANGELOG.md`
3. `v0.1.0` tag を作成する。
4. GitHub Release を作成する。
   - 初回公開の目的
   - 対応範囲
   - 既知の制限
   - 検証結果
5. crates.io 公開を行う場合は以下を実行する。
   - crate 名の可用性確認
   - `md-wiki` は既存 crate と衝突するため、package 名は `md-wiki-cli`、binary 名は `md-wiki` とする
   - `cargo publish --dry-run`
   - `cargo publish`
6. 公開後の smoke test を行う。
   - fresh clone
   - `cargo install --path .`
   - 最小 fixture で `md-wiki <INPUT> -o <OUT>`
   - 生成された `index.md`, `agent/index.md`, `fragments/_index.md` を確認する

### 完了条件

- GitHub Release `v0.1.0` が作成されている
- crates.io 公開する場合は `cargo install md-wiki-cli` で `md-wiki` binary を導入できる
- 公開後 smoke test が成功する

## 推奨 PR 分割

| PR | 内容 | 主な検証 |
|---|---|---|
| PR-1 | License / Cargo metadata / package exclude / `.gitignore` | `cargo package --locked`, `scripts/verify.sh` |
| PR-2 | 要件定義の整合性修正 | `cargo test --test docs`, `scripts/verify.sh` |
| PR-3 | README 改善 | `cargo test --test docs`, `scripts/verify.sh` |
| PR-4 | OSS 運用ファイル追加 | markdown review, `scripts/verify.sh` |
| PR-5 | CI 強化と品質履歴更新 | GitHub Actions, `scripts/verify.sh` |
| PR-6 | `v0.1.0` release prep | `cargo package --locked`, `cargo publish --dry-run`, smoke test |

## 最短公開ライン

公開を急ぐ場合は、以下を満たした時点で GitHub 公開は可能とする。

1. `LICENSE` がある
2. `Cargo.toml` に公開 metadata がある
3. package contents に内部作業ファイルが入らない
4. 1 MiB 超 Markdown の要件記述矛盾が解消されている
5. README に install と最小例がある
6. `scripts/verify.sh` が成功する
7. `cargo package --locked` が成功する

crates.io 公開は、上記に加えて `CHANGELOG.md`、`cargo publish --dry-run`、fresh install smoke test が完了してから行う。

## 公開判定チェックリスト

- [x] License decision is recorded
- [x] `LICENSE` exists
- [x] `Cargo.toml` has public package metadata
- [x] package contents are reviewed
- [x] `.context/` and local work files are excluded
- [x] README has installation instructions
- [x] README has a minimal example
- [x] README explains safety and limitations
- [x] requirements docs match current large Markdown behavior
- [x] `CONTRIBUTING.md` exists
- [x] `SECURITY.md` exists
- [x] `CHANGELOG.md` exists
- [x] issue / PR templates exist
- [x] `scripts/verify.sh` passes
- [x] `cargo package --locked` passes
- [x] CI passes on target OSes
- [x] quality score history is clean and current
- [x] `v0.1.0` release notes are drafted
- [ ] post-release smoke test passes
