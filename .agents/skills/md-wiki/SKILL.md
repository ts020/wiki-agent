---
name: md-wiki
description: Work on the md-wiki Rust CLI, Markdown wiki generation behavior, requirements, acceptance criteria, fragments, links, tags, indexes, backlinks, or tests.
---

# md-wiki Skill

When this skill is used, read these files from the repository root first:

- `.agents/core.md`
- `docs/要件定義/index.md`
- `docs/要件定義/13-受け入れ基準.md`

Before implementing, confirm whether the request is in v1 scope. Treat `docs/要件定義/` as authoritative, and do not implement items from `docs/要件定義/14-将来拡張.md` unless explicitly requested.

For behavior changes, add or update focused tests and run:

```sh
cargo test
```
