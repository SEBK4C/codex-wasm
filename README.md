# codex-wasm

OpenAI's Codex (codex-rs, the Rust CLI's agent core) compiled to WebAssembly (`wasm32-wasip1`), for a
browser tab. It runs in [cc-in-just-bash](https://git.bunny-sunfish.ts.net/k4c/cc-in-just-bash)'s tab next to
Claude Code: the same in-memory file system, the same just-bash shell and tools, switchable from the header.

| | |
|---|---|
| Codex | openai/codex at `build/CODEX_REV` (ce0323f, 2026-10-17) plus `patches/` (cfg gates, the entry crate) |
| Module | 43.6 MB (`wasm-opt -Oz`), 11.8 MB brotli; 31 WASI preview 1 imports and 13 `codex_host` imports |
| Host | `host/wasi.mjs` (WASI over a memfs volume), `host/codex-host.mjs` (processes, fetch, the tick loop) |
| Tests | `test/node-run.mjs` (Node, mock Responses API); cc-in-just-bash `test/codex-ui.e2e.mjs` (browser) |

## How it works

Codex is a tokio program with processes, sockets, TLS, a PTY, SQLite and OS sandboxes. In a tab:

- **No blocking.** The module is a WASI reactor. The tab calls `codex_start` once, then `codex_tick(ms)`
  whenever something happened or a timer is due; a tick runs the ready tasks of a current-thread tokio
  runtime by yielding (never parking) and returns. Host operations (a command, a request) return at once;
  when they progress, the tab calls `codex_host_wake(handle)`, which wakes the waiting task
  (`crates/codex-wasm-host`).
- **Processes** are `tokio::process` on WASI (`crates/tokio/src/process/wasi.rs`): program, arguments,
  environment and directory go to the tab, which runs them in just-bash. Exit codes are real: std's WASI
  `ExitStatus` is patched to carry one (`build/std-patch.py`, built with `-Z build-std`).
- **HTTP** is reqwest's native API with one change (`crates/reqwest/src/async_impl/wasi_fetch.rs`): every
  request goes to the tab's `fetch` (TLS, HTTP/2, redirects and decompression are the browser's).
  `codex-http-client` and everything above it is unchanged.
- **Files** are std's WASI file system over wasi-libc, which the tab implements on its memfs volume
  (`host/wasi.mjs`, one preopen `/`). std is patched for what WASI lacks: `current_exe`, PATH splitting,
  `home_dir`/`temp_dir`, file locks (one process per file system).
- **What a tab cannot have** compiles to stubs that fail with `Unsupported`: sockets (`crates/socket2`, socket
  types in the tokio fork), PTYs (`crates/portable-pty`; Codex falls back to pipes), OpenSSL, Unix sockets,
  the OS sandboxes. The tab is the sandbox: approvals are off and the sandbox mode is full access there.

The entry crate (`codex-rs/wasm`, in `patches/`) loads the config the standard way (`~/.codex/config.toml`),
starts one thread through `codex-core-api`, takes the page's messages (`{"type":"user","text","model"}`,
`interrupt`, `shutdown`) and emits Codex's protocol events as JSON.

## Build (on waffle, in podman)

```
build/setup.sh        # codex/ = openai/codex at CODEX_REV + patches/ (once)
build/release.sh      # syncs to waffle, builds wasm-small + wasm-opt into out/ (about 10 min from scratch)
build/node-test.sh /root/codex-wasm/out/codex.wasm   # one turn in Node: exec_command in just-bash, mock model
```

`build/Containerfile` is the image (Rust 1.95 as pinned by Codex, wasi-sdk, binaryen, std patched).
`build/cargo.sh` runs cargo for `wasm32-wasip1` with the patched std; `build/check-json.sh` and
`build/gated.py` were the porting loop (keep-going checks; widen `cfg(unix)` gates rustc blamed).
Changing Codex: edit in `codex/` (branch `wasm`), commit, then `git -C codex format-patch CODEX_REV..wasm -o ../patches`.

cc-in-just-bash takes the module with `./fetch-codex.sh` (from `waffle:/root/codex-wasm/out`).

## Known gaps

- **Code mode**: current models are `code_mode_only` (they call tools by writing JavaScript that runs in a
  separate V8 host process). The tab rewrites the catalog to `code_mode`, so Codex uses direct tools. A
  browser code-mode session (`ThreadManager::with_code_mode_session_provider`, the model's JavaScript in a
  sandboxed worker) would give these models their native tool mode.
- **SQLite state DB**: no file locking through WASI, so Codex runs without it (a warning; rollouts still
  go to `~/.codex/sessions`).
- **Resume**: a reopened chat starts a new Codex thread on the same files; the rollout is there but not
  resumed yet.
- **Injection check**: the tab's Jev check annotates Claude's Messages requests only, not Codex's.
- **Streaming output**: just-bash returns a command's output when it exits, so long commands show nothing
  until then.

## The subscription

Codex in the tab talks only to llm-beta's Codex route (`/api/v1/subscriptions/openai/codex`, the gateway's
held ChatGPT login for Seb's tailnet devices and the cc-agent origin). It is Codex's own code making
Codex's own requests (its `originator`, session and turn headers); the page itself asks the route for
nothing (the model list is the one Codex ships). Tests use the mock in `test/`, never the route.
