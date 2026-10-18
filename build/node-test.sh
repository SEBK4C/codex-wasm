#!/usr/bin/env bash
# Run the Node end-to-end test on waffle against the last module build.
#   build/node-test.sh [wasm profile dir, default wasm]
set -euo pipefail
cd "$(dirname "$0")/.."
rsync -a --delete --exclude node_modules host test package.json waffle:/root/codex-wasm/js/
ssh waffle "cd /root/codex-wasm/js && podman run --rm --network host --cpus 4 --memory 4g -e RUST_LOG=\${RUST_LOG:-warn} \
  -v \$PWD:/js:z -v /root/codex-wasm/cache/target/wasm32-wasip1/${1:-wasm}/codex_wasm.wasm:/codex.wasm:ro -w /js \
  docker.io/library/node:24-trixie sh -c '[ -d node_modules ] || npm install -s --no-audit --no-fund >/dev/null; node test/node-run.mjs /codex.wasm'"
