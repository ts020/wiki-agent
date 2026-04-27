#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

record_score=0
history_dir="docs/quality/history"
min_score="${MIN_QUALITY_SCORE:-90}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --record-score)
      record_score=1
      shift
      ;;
    --history)
      history_dir="$2"
      shift 2
      ;;
    --min-score)
      min_score="$2"
      shift 2
      ;;
    -h|--help)
      cat <<'USAGE'
Usage: scripts/verify.sh [--record-score] [--history DIR] [--min-score N]

Runs the full continuous verification gate:
  1. cargo fmt --check
  2. cargo clippy -- -D warnings
  3. cargo test
  4. cargo run --example quality_score -- --min-score N
  5. cargo run --example large_md_gate -- --mode normal --min-score 100

With --record-score, writes per-commit score reports under DIR.
USAGE
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

cargo fmt --check
cargo clippy -- -D warnings
cargo test

score_args=(--min-score "$min_score")
if [[ "$record_score" -eq 1 ]]; then
  score_args+=(--history "$history_dir")
fi

cargo run --quiet --example quality_score -- "${score_args[@]}"
cargo run --quiet --example large_md_gate -- \
  --mode normal \
  --work-dir target/large-md-gate \
  --report target/large-md-gate/report.json \
  --min-score 100
