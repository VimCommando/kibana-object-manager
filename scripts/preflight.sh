#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
export OPENSPEC_TELEMETRY=0

rust_checks() {
  local toolchain
  toolchain=$(awk -F '"' '/^channel =/ { print $2 }' rust-toolchain.toml)
  rustup run "$toolchain" cargo fmt --all -- --check
  rustup run "$toolchain" cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
  rustup run "$toolchain" cargo test --workspace --all-features --locked
  rustup run "$toolchain" cargo test -p kibana-sync --no-default-features --locked
  shellcheck scripts/*.sh crates/kibana-object-manager/scripts/*.sh
  python3 -m unittest discover -s scripts -p 'test_*.py'
}

docs_checks() {
  [[ "$(okf --version)" == "okf 0.2.7 "* ]] || { echo "Install okf 0.2.7" >&2; return 1; }
  [[ "$(openspec --version)" == "1.11.0" ]] || { echo "Install @fission-ai/openspec 1.11.0" >&2; return 1; }
  okf validate crates/kibana-object-manager/docs/
  python3 scripts/check_docs.py
  openspec validate --all --strict --no-interactive
}

case "${1:-all}" in
  all) rust_checks; docs_checks ;;
  rust) rust_checks ;;
  docs) docs_checks ;;
  msrv) rustup run 1.89.0 cargo check --workspace --all-targets --all-features --locked ;;
  *) echo "Usage: $0 [all|rust|docs|msrv]" >&2; exit 2 ;;
esac
