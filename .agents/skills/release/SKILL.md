---
name: release
description: Cut a tagged md-wiki release through cargo-dist. Bumps Cargo.toml + Cargo.lock, updates CHANGELOG.md, commits "Release vX.Y.Z", tags vX.Y.Z, pushes both atomically, waits for release.yml, and runs a curl-install smoke check. Invoke with `/release X.Y.Z` (no `v` prefix in the argument).
---

# release skill

Read `.agents/core.md` and `docs/RELEASING.md` from the repository root first. `docs/RELEASING.md` is the spec; this skill is the automation of that spec for `dist`-based releases.

## Argument

Single semver, e.g. `0.1.2`. Reject:

- Missing or non-semver.
- Starts with `v` (the tag is v-prefixed, the argument is not).
- Not strictly greater than the current `Cargo.toml` version.

Tag form is `v<X.Y.Z>`. Stay consistent — `dist`'s trigger accepts both bare and `v` forms, but mixing them across releases is confusing.

## 1. Preflight

Run in parallel; abort with a clear error on the first failure. Do not auto-fix.

- `git rev-parse --abbrev-ref HEAD` is `main`.
- `git status --porcelain` is empty.
- `git fetch origin` and `git rev-list --count HEAD..origin/main` is `0`.
- `git ls-remote --tags origin v<X.Y.Z>` empty AND `git rev-parse -q --verify refs/tags/v<X.Y.Z>` fails.
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --locked`
- `scripts/verify.sh` if it exists.
- `dist plan` runs cleanly *with the new version already in Cargo.toml*. Do this check after the edit step below.

## 2. Apply edits

1. `Cargo.toml`: set `version = "<X.Y.Z>"` under `[package]`.
2. Refresh `Cargo.lock`: `cargo build` (NOT `--locked`).
3. `CHANGELOG.md`:
   - Insert `## <X.Y.Z> - <today YYYY-MM-DD>` above the most recent prior version section.
   - If `## Unreleased` exists with bullets, move them in and drop the Unreleased header. Otherwise leave a `- TODO: summarize the user-visible change` placeholder so the maintainer must replace it.
   - `dist` reads this section as the GitHub Release body, so the bullets become user-facing notes.
4. Run `dist plan` and confirm it succeeds with no warnings. If it fails, the version bump or `dist-workspace.toml` is wrong — fix before continuing.

`docs/releases/v<X.Y.Z>.md` is **optional** with `dist` — it is no longer wired into the workflow. Only create it if the maintainer asks for hand-curated notes beyond the CHANGELOG bullets.

## 3. Review pause

Stage and show the diff:

```sh
git add Cargo.toml Cargo.lock CHANGELOG.md
git diff --cached
```

Then stop and ask: **go** / **edit** / **abort**.

- `go` — commit + tag + push.
- `edit` — pause so notes can be revised; then re-show the diff.
- `abort` — `git restore --staged --worktree Cargo.toml Cargo.lock CHANGELOG.md` and exit.

Do not proceed without an explicit answer.

## 4. Commit + tag + push

Per `.agents/core.md`: commit messages explain intent, do not enumerate files, and never include `Co-Authored-By` lines.

```sh
git commit -m "Release v<X.Y.Z>" -m "<short why-level summary drawn from the CHANGELOG entry>"
git tag v<X.Y.Z>
git push --atomic origin main v<X.Y.Z>
```

## 5. Watch the release workflow

```sh
sleep 5
run_id=$(gh run list --workflow release.yml --branch v<X.Y.Z> --json databaseId --jq '.[0].databaseId')
gh run watch "$run_id" --exit-status
```

On failure, fetch logs with `gh run view "$run_id" --log-failed` and report the failing phase (`plan`, `build-local`, `build-global`, `host`, or `announce`). Do not delete the tag.

## 6. Verify the release

1. **Asset count present.** `gh release view v<X.Y.Z> --json assets --jq '.assets | length'` returns a non-zero count that matches what `dist plan` enumerated (typically 13: 5 platform archives + 5 per-archive sha256 + 2 installers + `sha256.sum`, plus optionally `source.tar.gz` + its sha256 = 15). Use the `dist plan` output from step 2 as the source of truth.
2. **Latest pointer updated.** `gh api repos/ts020/wiki-agent/releases/latest --jq '.tag_name'` returns `v<X.Y.Z>`.
3. **Installer smoke test.**

   ```sh
   tmpdir=$(mktemp -d)
   curl --proto '=https' --tlsv1.2 -LsSf \
     https://github.com/ts020/wiki-agent/releases/download/v<X.Y.Z>/md-wiki-cli-installer.sh \
     | MD_WIKI_CLI_INSTALL_DIR="$tmpdir" sh
   "$tmpdir/md-wiki" --version  # expect: md-wiki <X.Y.Z>
   rm -rf "$tmpdir"
   ```

4. **Checksum verification ran.** `dist`'s installer prints checksum-verification output by default. If it is absent or shows a skip, that is a regression — report it.

## 7. Report

- Release URL: `https://github.com/ts020/wiki-agent/releases/tag/v<X.Y.Z>`
- Workflow run URL
- One-line confirmation: asset count matches plan, latest pointer flipped, smoke install reported the expected version.

## Failure modes

- **Preflight failure**: tell the user which check failed and the command to fix it. Do not auto-fix.
- **`dist plan` failure after the version bump**: likely a `dist-workspace.toml` misconfig or a Cargo.toml/tag mismatch in some other field. Show `dist plan`'s output verbatim and stop.
- **Push rejected**: another commit landed on `origin/main` between the preflight fetch and the push. Tell the user to `git pull --ff-only` and re-run.
- **Workflow failure mid-build**: report failing job logs. The tag exists on the remote with no assets — that is recoverable by fix-forward (patch version + new tag).
- **Asset count under plan**: rare; usually a flake. Re-run by either `gh workflow run release.yml -f tag=v<X.Y.Z>` (if dispatch is wired) or by deleting the failed-half release and re-pushing the tag.
