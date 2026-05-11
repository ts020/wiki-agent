# Releasing md-wiki

Release distribution is handled by [`dist`](https://github.com/axodotdev/cargo-dist) (formerly `cargo-dist`). The pipeline is driven entirely by git tags: pushing a semver-shaped tag fires `.github/workflows/release.yml`, which `dist` regenerated from `dist-workspace.toml`.

The recommended way to cut a release is the `/release` skill (`.agents/skills/release/`). The steps below are what that skill automates — useful both as documentation and as a manual fallback.

## 1. Preflight

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
scripts/verify.sh
dist plan
```

`dist plan` runs the same logic the CI will run, so a clean plan now is the best predictor of a green release.

## 2. Bump the crate version

`dist` requires `Cargo.toml`'s `version` to match the tag you push, so the version bump is unavoidable in this Pattern-B flow. Update:

- `Cargo.toml`: `version = "<X.Y.Z>"`
- `Cargo.lock`: refresh with `cargo build` (do not pass `--locked`)
- `CHANGELOG.md`: insert a `## <X.Y.Z> - <YYYY-MM-DD>` section above the previous one. `dist` reads this section and uses it as the GitHub Release body.

Optional: write `docs/releases/v<X.Y.Z>.md` if you want hand-curated notes that are richer than the CHANGELOG section. The `dist` workflow does not consume this file; it exists as a historical archive in the repository.

## 3. Commit + tag + push

```sh
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "Release v<X.Y.Z>"
git tag v<X.Y.Z>
git push --atomic origin main v<X.Y.Z>
```

`--atomic` keeps the commit and the tag together on the remote.

## 4. Watch the workflow

```sh
run_id=$(gh run list --workflow release.yml --branch v<X.Y.Z> --json databaseId --jq '.[0].databaseId')
gh run watch "$run_id" --exit-status
```

The workflow has four phases:

1. `plan` — validates `Cargo.toml` vs tag, emits the artifact matrix.
2. `build-local-artifacts` — runs on each target matrix entry and produces the platform tarball / zip and per-asset sha256.
3. `build-global-artifacts` — produces the shell and PowerShell installers, the `source.tar.gz`, and the combined `sha256.sum`.
4. `host` + `announce` — creates the GitHub Release with all artifacts and a body sourced from CHANGELOG.md.

## 5. Verify

```sh
# Asset count (5 tarballs/zips + 5 *.sha256 + source.tar.gz + source.tar.gz.sha256 + 2 installers + sha256.sum)
gh release view v<X.Y.Z> --json assets --jq '.assets | length'

# Latest pointer
gh api repos/ts020/wiki-agent/releases/latest --jq '.tag_name'

# End-to-end install smoke
tmpdir=$(mktemp -d)
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/ts020/wiki-agent/releases/download/v<X.Y.Z>/md-wiki-cli-installer.sh \
  | MD_WIKI_CLI_INSTALL_DIR="$tmpdir" sh
"$tmpdir/md-wiki" --version  # expect: md-wiki <X.Y.Z>
rm -rf "$tmpdir"
```

## Failure modes

- **`dist plan` rejects**: `Cargo.toml` version does not match the intended tag. Fix the manifest and re-run.
- **Tag already exists**: pick the next patch version.
- **Workflow fails mid-build**: check `gh run view <id> --log-failed`. Do not delete the tag unless you know nothing else uses it; prefer a follow-up patch.
- **Asset count wrong**: probably an upload race. Re-run the workflow against the same tag — `dist host --tag=v<X.Y.Z>` from a local environment can also reproduce.

## Updating dist itself

When a new `dist` version is released:

```sh
brew upgrade cargo-dist     # or: cargo install cargo-dist --force
dist init --yes             # re-runs init with the new version, regenerates release.yml
```

`dist init` is idempotent. Review the diff to `release.yml` before committing.
