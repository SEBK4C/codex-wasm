#!/usr/bin/env bash
# Build the shipped module on waffle: the wasm-small profile (fat LTO, opt-level z), then
# wasm-opt -Oz, into out/codex.wasm, with Codex's model list (out/models.json) and a manifest.
#   build/release.sh            (from the VM: syncs first, then runs on waffle)
set -euo pipefail
cd "$(dirname "$0")/.."
if [ "$(hostname)" != waffle ]; then
  build/sync.sh
  exec ssh waffle "cd /root/codex-wasm && build/release.sh"
fi
MEMORY=${MEMORY:-12g} build/cargo.sh build --profile wasm-small -p codex-wasm
mkdir -p out
build/run.sh wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
  --strip-debug --strip-producers /target/wasm32-wasip1/wasm-small/codex_wasm.wasm -o /target/codex-opt.wasm
cp cache/target/codex-opt.wasm out/codex.wasm
cp codex/codex-rs/models-manager/models.json out/models.json
rev=$(git -C codex rev-parse --short HEAD 2>/dev/null || echo unknown)
printf '{"codex_rs": "%s", "sha256": "%s", "bytes": %s, "built": "%s"}\n' "$rev" "$(sha256sum out/codex.wasm | cut -c1-64)" "$(stat -c %s out/codex.wasm)" "$(date -u +%FT%TZ)" > out/manifest.json
cat out/manifest.json
