#!/usr/bin/env bash
# Run the Node end-to-end test on waffle against the last module build.
#   build/node-test.sh [path of the module on waffle, default the wasm profile build]
set -euo pipefail
cd "$(dirname "$0")/.."
rsync -a --delete --exclude node_modules host test package.json waffle:/root/codex-wasm/js/ && rsync -a codex/codex-rs/models-manager/models.json waffle:/root/codex-wasm/js/codex/codex-rs/models-manager/
ssh waffle "cd /root/codex-wasm/js && podman run --rm --network host --cpus 4 --memory 4g -e RUST_LOG=\${RUST_LOG:-warn} -e MODEL=\${MODEL:-mock-model} \
  -v \$PWD:/js:z -v ${1:-/root/codex-wasm/cache/target/wasm32-wasip1/wasm/codex_wasm.wasm}:/codex.wasm:ro -w /js \
  docker.io/library/node:24-trixie sh -c '[ -d node_modules ] || npm install -s --no-audit --no-fund >/dev/null; node test/node-run.mjs /codex.wasm'"
