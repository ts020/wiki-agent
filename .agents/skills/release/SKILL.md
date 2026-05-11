---
name: release
description: Cut a new md-wiki release. Bumps Cargo.toml + Cargo.lock, updates CHANGELOG.md, scaffolds docs/releases/v<version>.md, commits, tags v<version>, pushes, waits for the release.yml workflow, and runs a curl-install smoke check. Invoke with `/release X.Y.Z` (no `v` prefix in the argument).
---

# release skill

Read `.agents/core.md` and `docs/RELEASING.md` from the repository root first. `docs/RELEASING.md` is the human-readable spec; this skill is the automation of that flow.

## Argument

The skill takes a single semver argument, e.g. `0.1.2`. Reject:

- Argument missing or non-semver.
- Argument starts with `v`. Tags are v-prefixed but the argument is the bare version.
- Argument is not strictly greater than the current `Cargo.toml` version.

The tag created will be `v<X.Y.Z>`. Tag form is intentionally standardized on the `v` prefix from 0.1.1 onwards — do not create bare-version tags going forward.

## 1. Preflight

Run these in parallel and abort with a clear error message on the first failure. Do not auto-fix; tell the user how to fix and stop.

- `git rev-parse --abbrev-ref HEAD` is `main`.
- `git status --porcelain` is empty (clean working tree).
- `git fetch origin` then `git rev-list --count HEAD..origin/main` is `0` (up to date).
- `git ls-remote --tags origin v<X.Y.Z>` returns nothing AND `git rev-parse -q --verify refs/tags/v<X.Y.Z>` fails (tag does not already exist locally or remotely).
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --locked`
- If `scripts/verify.sh` exists, run it.

## 2. Apply edits

1. `Cargo.toml`: set `version = "<X.Y.Z>"`.
2. Refresh `Cargo.lock`: `cargo build` (without `--locked`; the lock file must update).
3. `CHANGELOG.md`:
   - Insert a new section `## <X.Y.Z> - <today YYYY-MM-DD>` above the most recent existing version section.
   - If an `## Unreleased` section exists with bullets, move those bullets into the new section and remove the Unreleased header (do not leave an empty Unreleased section behind).
   - If there is no Unreleased content, leave a single placeholder bullet like `- TODO: summarize the user-visible change` so the maintainer must explicitly fill it in.
4. `docs/releases/v<X.Y.Z>.md`: create from this template, filling in date and copying the CHANGELOG bullets:

   ```
   # md-wiki v<X.Y.Z>

   Date: <today YYYY-MM-DD>

   <one-line summary — replace this>

   ## What changed

   <CHANGELOG bullets>

   ## Install

   ```sh
   curl -fsSL https://raw.githubusercontent.com/ts020/wiki-agent/main/install.sh | MD_WIKI_VERSION=v<X.Y.Z> sh
   ```
   ```

## 3. Review pause

Show the user the staged diff:

```sh
git add Cargo.toml Cargo.lock CHANGELOG.md docs/releases/v<X.Y.Z>.md
git diff --cached
```

Then stop and ask the user to choose:

- **go** — proceed to commit + tag + push.
- **edit** — pause so the user (or you, on instruction) can revise `docs/releases/v<X.Y.Z>.md` and the CHANGELOG section, then re-show the diff.
- **abort** — `git restore --staged --worktree Cargo.toml Cargo.lock CHANGELOG.md && rm -f docs/releases/v<X.Y.Z>.md` and exit.

Do not proceed without an explicit answer.

## 4. Commit + tag + push

Per `.agents/core.md`: commit messages explain intent, do not enumerate files, and never include `Co-Authored-By` lines.

```sh
git commit -m "Release v<X.Y.Z>" -m "<short why-level summary drawn from the release notes>"
git tag v<X.Y.Z>
git push --atomic origin main v<X.Y.Z>
```

`--atomic` keeps commit and tag together; either both land on the remote or neither does.

## 5. Watch the release workflow

The tag push fires `release.yml`. Find and watch it:

```sh
sleep 5
run_id=$(gh run list --workflow release.yml --branch v<X.Y.Z> --json databaseId --jq '.[0].databaseId')
gh run watch "$run_id" --exit-status
```

If `gh run watch` returns non-zero, fetch the failing job's logs (`gh run view "$run_id" --log-failed`) and report which target failed. Do not delete the tag — the user decides whether to push a patch.

## 6. Verify the release

Run all four checks; report any discrepancy.

1. **Assets attached:** `gh release view v<X.Y.Z> --json assets,isDraft --jq '.assets | length'` returns `6` (five tarballs + `checksums.txt`), and `isDraft` is `false`.
2. **Latest pointer updated:** `gh api repos/ts020/wiki-agent/releases/latest --jq '.tag_name'` returns `v<X.Y.Z>`.
3. **Curl install smoke test:**

   ```sh
   tmpdir=$(mktemp -d)
   curl -fsSL https://raw.githubusercontent.com/ts020/wiki-agent/main/install.sh \
     | MD_WIKI_INSTALL_DIR="$tmpdir" MD_WIKI_VERSION=v<X.Y.Z> sh
   "$tmpdir/md-wiki" --version  # expect: md-wiki <X.Y.Z>
   rm -rf "$tmpdir"
   ```

4. **Checksum line printed:** the install output above must include `Checksum verified: md-wiki-*.tar.gz`. If it instead shows `Skipping checksum verification`, that is a regression — report it.

## 7. Report

Output to the user:

- Release URL: `https://github.com/ts020/wiki-agent/releases/tag/v<X.Y.Z>`
- Workflow run URL
- One-line confirmation: assets count, latest pointer, smoke version match.

## Failure modes

- **Preflight failure:** report which check failed and the exact command the user can run to fix; do not auto-fix.
- **Push rejected:** another commit landed on `origin/main` between the preflight fetch and the push. Tell the user to `git pull --ff-only` and re-run.
- **Workflow failure:** the tag exists on the remote but no assets. Report the failing job. Do not delete the tag. The user can either fix forward (patch + new tag) or delete the tag manually if it was a misfire.
- **Asset count wrong:** the workflow succeeded but fewer than 6 assets. Probably an upload race. Re-run `gh workflow run release.yml -f tag=v<X.Y.Z>` to backfill.
