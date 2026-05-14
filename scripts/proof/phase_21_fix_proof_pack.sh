#!/usr/bin/env bash
set -euo pipefail

EVE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKDIR="$(mktemp -d)"

cleanup() {
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

run_case() {
  local name="$1"
  local setup_fn="$2"
  local command_fn="$3"

  local case_dir="$WORKDIR/$name"
  "$setup_fn" "$case_dir"
  local output
  output="$("$command_fn" "$case_dir" 2>&1)"
  printf '=== %s ===\n%s\n\n' "$name" "$output"
}

setup_missing_ci() {
  local root="$1"
  cargo new "$root" --lib >/dev/null
}

setup_missing_smoke() {
  local root="$1"
  cargo new "$root" --lib >/dev/null
  rm -f "$root/tests/eve_smoke.rs"
}

setup_readme_missing_validation() {
  local root="$1"
  cargo new "$root" --lib >/dev/null
  printf '# Proof Pack\n' >"$root/README.md"
}

setup_simple_missing_module() {
  local root="$1"
  cargo new "$root" --lib >/dev/null
  printf 'mod missing_module;\n' >"$root/src/lib.rs"
}

setup_unknown_empty_project() {
  local root="$1"
  mkdir -p "$root"
}

run_fix_command() {
  local root="$1"
  (
    cd "$EVE_DIR"
    cargo run -- fix "$root" --apply --no-llm
  )
}

run_fix_ci() {
  local root="$1"
  (
    cd "$EVE_DIR"
    cargo run -- fix "$root" --only ci --apply --no-llm
  )
}

run_fix_tests() {
  local root="$1"
  (
    cd "$EVE_DIR"
    cargo run -- fix "$root" --only tests --apply --no-llm
  )
}

run_fix_docs() {
  local root="$1"
  (
    cd "$EVE_DIR"
    cargo run -- fix "$root" --only docs --apply --no-llm
  )
}

run_fix_cargo_check() {
  local root="$1"
  (
    cd "$EVE_DIR"
    cargo run -- fix "$root" --only cargo-check --apply --no-llm
  )
}

run_case "missing_ci" setup_missing_ci run_fix_ci
run_case "missing_smoke_test" setup_missing_smoke run_fix_tests
run_case "readme_missing_validation" setup_readme_missing_validation run_fix_docs
run_case "simple_missing_module" setup_simple_missing_module run_fix_cargo_check
run_case "unknown_empty_project" setup_unknown_empty_project run_fix_command

printf 'Proof pack completed under %s\n' "$WORKDIR"
