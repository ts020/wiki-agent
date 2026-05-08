# Contributing to md-wiki

md-wiki の仕様判断は `docs/要件定義/` を正とします。実装と README が要件定義と矛盾する場合は、要件定義を更新するか、実装を要件へ戻してください。

## Scope

- 対象はローカル Markdown からオフライン wiki を生成する CLI です
- 入力 Markdown は変更しません
- AI、LLM、embedding、外部 API、ネットワークサービスには依存しません
- watch mode、差分更新、HTML 出力、全文検索 index は v0.1.0 の対象外です

## Development Commands

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
scripts/verify.sh
```

公開前の検証では以下も実行します。

```sh
cargo test --locked
cargo package --dry-run
cargo package --no-verify --list
```

## Testing Expectations

- Markdown 生成挙動を変える場合は、先に failing test を追加してから実装してください
- 仕様変更は `docs/要件定義/` とテストを同じ変更に含めてください
- README の利用者向け説明を変える場合は `tests/docs.rs` の整合性テストも更新してください
- 巨大 Markdown や agentic search 出力に触れる場合は、通常 gate と該当 example gate を実行してください

## Pull Request Checklist

- [ ] 変更は v1 / v0.1.0 scope に収まっている
- [ ] 要件定義、README、実装が矛盾していない
- [ ] 新しい挙動にはテストがある
- [ ] `cargo fmt --check` が成功する
- [ ] `cargo clippy -- -D warnings` が成功する
- [ ] `cargo test` が成功する
- [ ] 公開物に `.context/`, `.agents/`, `.claude/` などのローカル作業ファイルが入らない

