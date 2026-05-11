# Releasing md-wiki

リリース作業の手順をまとめます。バイナリ配布は `.github/workflows/release.yml` が担当し、対応プラットフォームへの cross build と GitHub Release への asset upload を自動化します。

## 1. リリース前の確認

```sh
cargo test --locked
cargo clippy -- -D warnings
cargo fmt --check
scripts/verify.sh
cargo package --locked
cargo package --no-verify --list
sh -n install.sh
```

`cargo package --no-verify --list` の出力に `.context/`、`.agents/`、`.claude/`、ローカル作業履歴が含まれていないことを確認します。

## 2. バージョン更新

- `Cargo.toml` の `version`
- `CHANGELOG.md` に新セクションを追加
- `docs/releases/v<version>.md` を作成（任意。あれば Release notes として採用）

`Cargo.lock` を同じコミットで更新します。

```sh
cargo build
git add Cargo.toml Cargo.lock CHANGELOG.md docs/releases/v<version>.md
git commit -m "Release v<version>"
```

## 3. タグ付け & push

タグ名は `0.1.0` でも `v0.1.0` でも構いません。release workflow は両方を受け付けます。

```sh
git tag 0.1.0      # or: git tag v0.1.0
git push origin main
git push origin 0.1.0
```

## 4. ワークフロー実行

タグ push でワークフローが自動起動します。手動で再実行する場合:

```sh
gh workflow run release.yml -f tag=0.1.0
```

完了後、[Releases](https://github.com/ts020/wiki-agent/releases) に以下が並ぶことを確認します。

- `md-wiki-x86_64-unknown-linux-gnu.tar.gz`
- `md-wiki-aarch64-unknown-linux-gnu.tar.gz`
- `md-wiki-x86_64-apple-darwin.tar.gz`
- `md-wiki-aarch64-apple-darwin.tar.gz`
- `md-wiki-x86_64-pc-windows-msvc.tar.gz`
- `checksums.txt`

## 5. インストール確認

少なくとも 1 プラットフォームで curl 経由インストールが通ることを確認します。

```sh
curl -fsSL https://raw.githubusercontent.com/ts020/wiki-agent/main/install.sh | MD_WIKI_VERSION=<tag> sh
md-wiki --version
```

## 6. crates.io 公開（任意）

```sh
cargo publish --dry-run
cargo publish
```

`md-wiki-cli` の package 名で公開され、binary 名は `md-wiki` のままです。

## トラブルシューティング

- **タグ push で workflow が起動しない**: タグ名が `^v?\d+\.\d+\.\d+` パターンに一致しているか確認。`workflow_dispatch` で `tag=<name>` を渡して再実行できます。
- **aarch64-linux ビルドが失敗する**: cross linker 設定は `gcc-aarch64-linux-gnu` パッケージに依存します。matrix の `apt` / `linker` 項目が抜けていないか確認。
- **asset upload が 0 個になる**: 既存 Release があるとき、workflow は upload のみを実行します。`gh release view <tag>` で asset を確認し、必要なら `gh release delete-asset` で再生成してください。
