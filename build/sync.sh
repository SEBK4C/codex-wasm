#!/usr/bin/env bash
# Copy the working tree (codex checkout + crates + build scripts) to waffle.
set -euo pipefail
cd "$(dirname "$0")/.."
rsync -a --delete --exclude .git --exclude target codex/ waffle:/root/codex-wasm/codex/
rsync -a --delete crates/ waffle:/root/codex-wasm/crates/
rsync -a build/ waffle:/root/codex-wasm/build/
