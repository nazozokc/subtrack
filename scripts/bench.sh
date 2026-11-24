#!/usr/bin/env bash
# Reproducible CLI benchmarks for subtrack. No extra dependencies.
#
# Usage:
#   cargo build --release --manifest-path rust/Cargo.toml
#   scripts/bench.sh
#
# Overrides: SUBTRACK_BIN, SUBTRACK_BENCH_COUNT, SUBTRACK_BENCH_RUNS,
#            SUBTRACK_TODAY, SUBTRACK_BENCH_KEEP=1
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${SUBTRACK_BIN:-$ROOT/rust/target/release/subtrack}"
COUNT="${SUBTRACK_BENCH_COUNT:-60}"
RUNS="${SUBTRACK_BENCH_RUNS:-7}"
TODAY="${SUBTRACK_TODAY:-2026-11-24}"
PASSPHRASE="bench-passphrase"

if [[ ! -x "$BIN" ]]; then
  echo "Binary not found: $BIN" >&2
  echo "Build first: cargo build --release --manifest-path rust/Cargo.toml" >&2
  exit 1
fi

TMP="$(mktemp -d -t subtrack-bench.XXXXXX)"
if [[ "${SUBTRACK_BENCH_KEEP:-0}" == "1" ]]; then
  trap 'echo "Benchmark fixtures kept at: $TMP"' EXIT
else
  trap 'rm -rf "$TMP"' EXIT
fi

DB_DIR="$TMP/scratch"
mkdir -p "$DB_DIR"

# Two `date` calls are part of every sample; subtract the no-op cost so the
# reported numbers stay close to the Python/hyperfine-style measurements.
cal_start="$(date +%s%N)"
:
cal_end="$(date +%s%N)"
OVERHEAD_MS="$(((cal_end - cal_start) / 1000000))"

run_with_env() {
  if [[ -n "${MODE_PASSPHRASE:-}" ]]; then
    SUBSC_CLI_DB_DIR="$DB_DIR" SUBSC_CLI_DB_PASSPHRASE="$PASSPHRASE" \
      SUBTRACK_TODAY="$TODAY" "$BIN" "$@"
  else
    SUBSC_CLI_DB_DIR="$DB_DIR" SUBTRACK_TODAY="$TODAY" "$BIN" "$@"
  fi
}

measure() {
  local label="$1"
  shift
  local times=()
  for ((i = 0; i < RUNS; i++)); do
    local start end
    start="$(date +%s%N)"
    run_with_env "$@" >/dev/null 2>&1 || true
    end="$(date +%s%N)"
    elapsed="$(((end - start) / 1000000 - OVERHEAD_MS))"
    times+=("$((elapsed > 0 ? elapsed : 0))")
  done
  printf '%s\n' "${times[@]}" | sort -n | awk -v label="$label" \
    '{ a[NR] = $1 } END { printf "  %-26s min=%5dms  median=%5dms\n", label, a[1], a[int((NR + 1) / 2)] }'
}

populate() {
  local dir="$1"
  local marker="${2:-}"
  DB_DIR="$dir"
  MODE_PASSPHRASE=""
  mkdir -p "$DB_DIR"
  if [[ -n "$marker" ]]; then
    MODE_PASSPHRASE="1"
    printf '%s\n' "$marker" >"$DB_DIR/.key.kdf"
    dd if=/dev/zero of="$DB_DIR/.key.salt" bs=32 count=1 status=none
  fi
  for ((i = 1; i <= COUNT; i++)); do
    run_with_env add --name "Service $i" --price "$((5 + i % 100))" \
      --currency USD --cycle monthly --tags work,tools >/dev/null 2>&1
  done
}

echo "subtrack benchmark — $COUNT subscriptions, $RUNS runs, binary: $BIN"
echo "  (--version / --help)"
measure "--version" --version
measure "--help" --help

populate "$TMP/keyfile" ""
echo "key-file encryption (.key):"
measure "list --json" list --json
measure "list (text)" list
measure "summary" summary
measure "report --json" report --json

populate "$TMP/passphrase-fast" "2"
MODE_PASSPHRASE="1"
DB_DIR="$TMP/passphrase-fast"
echo "passphrase encryption (Fast, N=2^15):"
measure "list --json" list --json
measure "report --json" report --json

populate "$TMP/passphrase-legacy" "1"
MODE_PASSPHRASE="1"
DB_DIR="$TMP/passphrase-legacy"
echo "passphrase encryption (Legacy, N=2^17):"
measure "list --json" list --json
measure "report --json" report --json
