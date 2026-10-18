#!/usr/bin/env bash
# cargo for wasm32-wasip1 with the patched std (on waffle): build/cargo.sh check -p codex-wasm
set -euo pipefail
cmd=$1; shift
exec "$(dirname "$0")/run.sh" cargo "$cmd" -Zbuild-std=std,panic_abort --target wasm32-wasip1 "$@"
