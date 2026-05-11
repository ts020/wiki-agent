## Summary

Describe the change.

## Requirements

- [ ] Checked the relevant `docs/要件定義/` section
- [ ] Updated requirements docs when behavior changed
- [ ] Updated README or release docs when user-facing behavior changed

## Tests

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -- -D warnings`
- [ ] `cargo test`
- [ ] `scripts/verify.sh`

## Release / Package Check

- [ ] Public package metadata remains valid
- [ ] No local work files are included in the package
- [ ] `cargo package --locked` passes when this is release-facing
