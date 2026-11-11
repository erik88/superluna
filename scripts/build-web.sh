#!/usr/bin/env bash
# Build the browser version into web/dist. Serve it with: python3 -m http.server -d web/dist 8080
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --target wasm32-unknown-unknown --profile web -p ns-frontend
mkdir -p web/dist
cp web/index.html web/mq_js_bundle.js web/dist/
cp target/wasm32-unknown-unknown/web/platformer.wasm web/dist/
ls -lh web/dist
