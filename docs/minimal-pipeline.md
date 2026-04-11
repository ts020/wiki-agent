# 最小貫通パイプライン

## 目的

パイプライン全体（CLI → scan → analyze → generate → Markdown出力）を最短で貫通させる。
各段は最も単純な実装から始め、動くことを確認してから育てる。

## 受け入れ基準（要件定義 セクション12 から）

最小パイプラインは以下をすべて満たした時点で貫通とする:

- [ ] index.md が生成される
- [ ] 複数ノードが生成される
- [ ] ノード構造（FR-09: Summary, Key files, Responsibilities, Related, Read next）が存在する
- [ ] 除外対象（.git, node_modules, dist, build, target）が無視される
- [ ] 任意ノード単体で理解可能
- [ ] コードベースが変更されない

## 各段の最小スコープ

### CLI

- 引数: 対象ディレクトリ（省略時カレント）、出力先（省略時 ./repo-wiki）
- それ以外のオプションは後

### Scanner

- 対象ディレクトリを再帰走査し、ファイルパスを収集する
- 除外対象をスキップする
- 読めないファイルはスキップする
- 出力: ScanResult（root + 相対パスのリスト）

### Analyzer

- ScanResult からどのノードを作るか決める
- 最小の分割方針: index + ルート直下の子ディレクトリごとに1ノード
- 深い階層の分割は後。まず1段で通す
- 出力: WikiNode の骨格（title, output_path, scope_files のみ確定。内容は空）

### Generator

- 各ノードの scope_files を読み、Claude API に投げて WikiNode の内容を埋める
- WikiNode を Markdown に変換して書き出す
- **最初のステップ: Claude API なしで固定テンプレートから生成する**
- Claude API 連携は、パイプラインが貫通した後に差し替える

### Markdown 出力

- WikiNode → Markdown 変換は1つの関数で行う
- 全ノード同じフォーマット

## 2段階で通す

### Step 1: Claude API なし

```
CLI → Scanner → Analyzer → Generator(テンプレート) → Markdown
```

パイプラインの配管が正しく繋がっていることを確認する。
出力される Markdown の内容は仮だが、構造（ファイル数、ノード構成、リンク）は本物。

### Step 2: Claude API 接続

```
CLI → Scanner → Analyzer → Generator(Claude API) → Markdown
```

Generator 内部のテンプレート生成を Claude API 呼び出しに差し替える。
パイプラインの形は変わらない。Generator の中身だけが変わる。

## やらないこと（最小パイプライン時点では）

- 深い階層のノード分割（2段目以降）
- overview/ や development/ カテゴリへの分類
- トークン量の制御・分割送信
- エラーリトライ
- 進捗表示
- 設定ファイル
