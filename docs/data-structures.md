# データ構造設計

## 要件から導かれる構造

要件定義の出力物は以下:

```
repo-wiki/
├── index.md
├── overview/
├── directories/
└── development/
```

各ノード（= 1つの Markdown ファイル）は以下を含む（FR-09）:
- Summary
- Key files
- Responsibilities
- Related
- Read next

## 型定義

```rust
/// wikiツリーの1ノード。
/// index.md も directories/src.md も同じ型。特殊ケースはない。
struct WikiNode {
    title: String,
    output_path: PathBuf,
    summary: String,
    key_files: Vec<KeyFile>,
    responsibilities: Vec<String>,
    related: Vec<PathBuf>,
    read_next: Vec<PathBuf>,
}

struct KeyFile {
    path: PathBuf,
    description: String,
}
```

これが出力の単位であり、このプロジェクトの中心となるデータ構造。

## ツリー構造

WikiNode 同士は `read_next` と `related` のパスで繋がり、任意の深さの木構造を形成する。
深さは型で制限されない — `read_next` の先もまた WikiNode であり、その先にもまた `read_next` がある。

```
index.md
├── overview/tech.md
├── overview/arch.md
├── directories/src.md
│   ├── directories/src/handlers.md
│   │   ├── directories/src/handlers/auth.md
│   │   └── directories/src/handlers/api.md
│   ├── directories/src/models.md
│   └── directories/src/services.md
│       ├── directories/src/services/payment.md
│       └── directories/src/services/notification.md
├── directories/lib.md
└── development/setup.md
```

`read_next` = 深掘り（親→子）、`related` = 横移動（兄弟・別枝）。

大規模リポジトリでは階層が深くなる。小さなリポジトリでは浅くなる。
型は同じまま、コードベースの情報量に応じてツリーが伸縮する。

## 判断の根拠

- **フィールドは FR-09 から直接導出した。** 要件にあるものだけ入れた
- **ノードの種類を区別する enum やフラグは持たない。** index も overview も directories も同じ WikiNode。スコープが違うだけ
- **パイプラインの中間構造はここでは定義しない。** 実装時に必要になったら追加する
