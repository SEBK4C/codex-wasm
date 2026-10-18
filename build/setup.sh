#!/usr/bin/env bash
# Check out Codex at the pinned commit with this repo's patches (patches/) applied, as codex/.
#   build/setup.sh            (codex/ must not exist yet)
set -euo pipefail
cd "$(dirname "$0")/.."
REV=$(cat build/CODEX_REV)
[ -e codex ] && { echo "codex/ exists; remove it first" >&2; exit 1; }
git clone -q --filter=blob:none https://github.com/openai/codex.git codex
git -C codex checkout -q -b wasm "$REV"
git -C codex -c user.name=codex-wasm -c user.email=codex-wasm@localhost am -q ../patches/*.patch
git -C codex log --oneline -3
