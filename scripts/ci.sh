#!/usr/bin/env bash
# Everything CI checks. Run locally before pushing: scripts/ci.sh
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "format"
cargo fmt --all --check

step "lint"
cargo clippy --workspace --all-targets -- -D warnings

step "tests (unit + feel spec)"
cargo test --workspace --quiet

step "web build (everything must compile for the browser)"
cargo build --quiet --target wasm32-unknown-unknown --profile web -p ns-frontend
cargo build --quiet --target wasm32-unknown-unknown --release -p ns-web-check

step "replays (desktop)"
cargo run --quiet --release -p ns-headless --bin ns -- check

step "replays (WebAssembly, must match desktop)"
node scripts/web-check.mjs

printf '\n\033[1;32mall checks passed\033[0m\n'
