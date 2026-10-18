#!/usr/bin/env bash
# cargo for wasm32-wasip1 with the patched std (on waffle): build/cargo.sh check -p codex-wasm
# When the build image (and so the patched std sources) changed since the last build, std's
# fingerprints are dropped first: cargo does not notice changed rust-src files on its own.
set -euo pipefail
cd "$(dirname "$0")/.."
image=$(grep -o 'codex-wasm-build:[0-9]*' build/run.sh | head -1)
mkdir -p cache/target
if [ "$(cat cache/target/.std-image 2>/dev/null)" != "$image" ]; then
  rm -rf cache/target/wasm32-wasip1/*/.fingerprint/std-* cache/target/wasm32-wasip1/*/.fingerprint/core-* cache/target/wasm32-wasip1/*/.fingerprint/alloc-*
  echo "$image" > cache/target/.std-image
fi
cmd=$1; shift
exec build/run.sh cargo "$cmd" -Zbuild-std=std,panic_abort --target wasm32-wasip1 "$@"
