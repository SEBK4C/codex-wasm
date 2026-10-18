#!/usr/bin/env bash
# Run a command in the build image on waffle, over /root/codex-wasm/codex.
# Usage (on waffle): build/run.sh <cmd...>   e.g. build/run.sh cargo check -p codex-wasm --target wasm32-wasip1
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p cache/cargo-registry cache/cargo-git cache/target
exec podman run --rm -e CARGO_TARGET_WASM32_WASIP1_RUSTFLAGS="--cfg tokio_unstable -Z wasi-exec-model=reactor" --network host --cpus "${CPUS:-10}" --memory "${MEMORY:-7g}" \
  -v "$PWD/codex:/src/codex:z" -v "$PWD/crates:/src/crates:z" -v "$PWD/cache/cargo-registry:/usr/local/cargo/registry:z" \
  -v "$PWD/cache/cargo-git:/usr/local/cargo/git:z" -v "$PWD/cache/target:/target:z" \
  -e CARGO_TARGET_DIR=/target -e CARGO_BUILD_JOBS="${JOBS:-8}" -e CARGO_TERM_COLOR=never \
  -w /src/codex/codex-rs codex-wasm-build:5 "$@"
