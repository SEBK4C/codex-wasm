// The tab's side of Codex on wasm32-wasip1 (codex-wasm-host in Rust): processes, HTTP, the
// page's messages, and the loop that drives the module.
//
// Every import answers at once. Work that takes time (a command in the shell, a fetch) runs
// in JavaScript; when something happens on a handle, the host calls the module's
// `codex_host_wake(handle)` and schedules a tick. A tick (`codex_tick`) runs the module's
// ready tasks for a few milliseconds and returns, so the tab's event loop never stalls.
import { createWasi, WasiExit } from "./wasi.mjs";

const PENDING = -1;
const enc = new TextEncoder(), dec = new TextDecoder();
// Headers a browser sets itself (fetch refuses or ignores them).
const FORBIDDEN = new Set(["host", "connection", "content-length", "accept-encoding", "user-agent", "keep-alive", "transfer-encoding", "te", "trailer", "upgrade", "expect", "cookie", "origin", "referer"]);

/**
 * @param {object} o
 * @param {object} o.fs memfs-style fs for WASI
 * @param {Record<string,string>} o.env the module's environment
 * @param {(spec: {program:string,args:string[],env:object,cwd:string}, io: {stdin: Uint8Array, signal: AbortSignal}) =>
 *         Promise<{stdout: string|Uint8Array, stderr: string|Uint8Array, exitCode: number}>} o.spawn
 * @param {typeof fetch} o.fetch
 * @param {(message: object) => void} o.onMessage what the module emits
 * @param {(line: string) => void} [o.log] module stderr and host notes
 */
export function createCodexHost({ fs, env, spawn, fetch, onMessage, log = () => {} }) {
  let exports = null, memory = null, stopped = false, timer = null, timerAt = Infinity;
  const handles = new Map();
  let nextHandle = 1;
  const inbox = [];
  let stderrLine = "";

  const wasi = createWasi({
    fs, env,
    stdout: (b) => log(dec.decode(b)),
    stderr: (b) => {
      stderrLine += dec.decode(b);
      const i = stderrLine.lastIndexOf("\n");
      if (i >= 0) { log(stderrLine.slice(0, i + 1)); stderrLine = stderrLine.slice(i + 1); }
    },
  });
  const u8 = () => new Uint8Array(memory.buffer);
  const readJson = (ptr, len) => JSON.parse(dec.decode(u8().slice(ptr, ptr + len)));
  // Copy a sized answer: the full length is returned; bytes are copied only when they fit.
  const answer = (bytes, ptr, cap) => { if (bytes.length <= cap) u8().set(bytes, ptr); return bytes.length; };
  const take = (chunks, ptr, cap) => {
    let n = 0;
    while (chunks.length && n < cap) {
      const c = chunks[0];
      const k = Math.min(c.length, cap - n);
      u8().set(c.subarray(0, k), ptr + n);
      n += k;
      if (k === c.length) chunks.shift(); else chunks[0] = c.subarray(k);
    }
    return n;
  };

  function wake(h) {
    if (stopped || !exports) return;
    try { exports.codex_host_wake(h); } catch (e) { return fail(e); }
    schedule(0);
  }

  // --- processes --------------------------------------------------------------------------
  function startProcess(h, p) {
    if (p.started) return;
    p.started = true;
    const stdin = concat(p.stdin);
    Promise.resolve()
      .then(() => spawn(p.spec, { stdin, signal: p.abort.signal }))
      .then((r) => {
        if (p.exit !== null) return;
        const out = toBytes(r.stdout), err = toBytes(r.stderr);
        if (out.length) p.out[1].push(out);
        if (err.length) p.out[2].push(err);
        p.exit = r.exitCode ?? 0;
      }, (e) => {
        if (p.exit !== null) return;
        p.out[2].push(enc.encode(`${e?.message ?? e}\n`));
        p.exit = 127;
      })
      .finally(() => { p.eof = true; wake(h); });
  }

  // --- HTTP -------------------------------------------------------------------------------
  function startHttp(h, spec, body) {
    const r = { head: null, chunks: [], eof: false, failed: false, abort: new AbortController() };
    handles.set(h, r);
    const headers = new Headers();
    for (const [k, v] of spec.headers ?? []) if (!FORBIDDEN.has(k.toLowerCase())) headers.append(k, v);
    const method = spec.method ?? "GET";
    (async () => {
      let res;
      try {
        res = await fetch(spec.url, { method, headers, body: method === "GET" || method === "HEAD" ? undefined : body, signal: r.abort.signal });
      } catch (e) {
        r.head = enc.encode(JSON.stringify({ error: `fetch ${spec.url}: ${e?.message ?? e}` }));
        r.eof = true;
        return wake(h);
      }
      // fetch has already decoded the body, so its encoding and length no longer apply.
      const hs = [];
      for (const [k, v] of res.headers) if (k !== "content-encoding" && k !== "content-length") hs.push([k, v]);
      r.head = enc.encode(JSON.stringify({ status: res.status, headers: hs, url: res.url || spec.url }));
      wake(h);
      if (!res.body) { r.eof = true; return wake(h); }
      const reader = res.body.getReader();
      try {
        for (;;) {
          const { value, done } = await reader.read();
          if (done) break;
          if (value?.length) { r.chunks.push(value); wake(h); }
        }
      } catch (e) {
        if (!r.abort.signal.aborted) { r.failed = true; log(`http body ${spec.url}: ${e?.message ?? e}\n`); }
      }
      r.eof = true;
      wake(h);
    })();
  }

  const host = {
    proc_spawn(ptr, len) {
      const spec = readJson(ptr, len);
      const h = nextHandle++;
      handles.set(h, { spec, stdin: [], stdinClosed: false, out: { 1: [], 2: [] }, eof: false, exit: null, started: false, abort: new AbortController() });
      return h;
    },
    proc_write(h, ptr, len) {
      const p = handles.get(h);
      if (!p || p.stdinClosed || p.started) return -32;
      p.stdin.push(u8().slice(ptr, ptr + len));
      return len;
    },
    proc_close_stdin(h) {
      const p = handles.get(h);
      if (!p) return;
      p.stdinClosed = true;
      startProcess(h, p);
    },
    proc_read(h, fd, ptr, cap) {
      const p = handles.get(h);
      if (!p) return 0;
      startProcess(h, p);
      const chunks = p.out[fd];
      if (!chunks) return 0;
      if (chunks.length) return take(chunks, ptr, cap);
      return p.eof ? 0 : PENDING;
    },
    proc_wait(h) {
      const p = handles.get(h);
      if (!p) return 0;
      startProcess(h, p);
      return p.exit ?? PENDING;
    },
    proc_kill(h) {
      const p = handles.get(h);
      if (!p || p.exit !== null) return;
      p.abort.abort();
      p.exit = 137;
      p.eof = true;
      queueMicrotask(() => wake(h));
    },
    proc_drop(h) {
      const p = handles.get(h);
      if (p?.exit === null && p.started) p.abort.abort();
      handles.delete(h);
    },
    http_start(sp, sl, bp, bl) {
      const spec = readJson(sp, sl);
      const h = nextHandle++;
      startHttp(h, spec, bl ? u8().slice(bp, bp + bl) : undefined);
      return h;
    },
    http_head(h, ptr, cap) {
      const r = handles.get(h);
      if (!r) return answer(enc.encode('{"error":"unknown request"}'), ptr, cap);
      return r.head ? answer(r.head, ptr, cap) : PENDING;
    },
    http_read(h, ptr, cap) {
      const r = handles.get(h);
      if (!r) return 0;
      if (r.chunks.length) return take(r.chunks, ptr, cap);
      if (r.failed) return -2;
      return r.eof ? 0 : PENDING;
    },
    http_drop(h) {
      const r = handles.get(h);
      if (r && !r.eof) r.abort.abort();
      handles.delete(h);
    },
    take_message(ptr, cap) {
      if (!inbox.length) return PENDING;
      const n = answer(inbox[0], ptr, cap);
      if (n <= cap) inbox.shift();
      return n;
    },
    emit(ptr, len) {
      let m;
      try { m = readJson(ptr, len); } catch (e) { return log(`bad message from codex: ${e}\n`); }
      queueMicrotask(() => onMessage(m));
    },
  };

  // --- driving ----------------------------------------------------------------------------
  function fail(e) {
    if (stopped) return;
    stopped = true;
    clearTimeout(timer);
    if (e instanceof WasiExit) onMessage({ type: "exit", code: e.code });
    else onMessage({ type: "crash", message: String(e?.stack ?? e) });
  }
  function schedule(delay) {
    if (stopped) return;
    const at = performance.now() + delay;
    if (timer !== null && timerAt <= at) return;
    clearTimeout(timer);
    timerAt = at;
    timer = setTimeout(tick, delay);
  }
  // While requests or processes are open the runtime may also wait on its own timers (stream
  // idle timeouts, retries), so idle ticks keep coming, just slower.
  function tick() {
    timer = null; timerAt = Infinity;
    if (stopped) return;
    let status;
    try { status = exports.codex_tick(15); } catch (e) { return fail(e); }
    if (status === 0) { stopped = true; return onMessage({ type: "exit", code: 0 }); }
    schedule(status === 2 ? 0 : handles.size ? 20 : 100);
  }

  return {
    imports(module) {
      const imports = { wasi_snapshot_preview1: wasi.imports, codex_host: host };
      // Anything else the module names (it should not) answers "not supported".
      for (const i of WebAssembly.Module.imports(module)) {
        imports[i.module] ??= {};
        if (i.kind === "function" && !(i.name in imports[i.module])) {
          imports[i.module][i.name] = () => { log(`unsupported import ${i.module}.${i.name}\n`); return 52; };
        }
      }
      return imports;
    },
    attach(instance) {
      exports = instance.exports;
      memory = exports.memory;
      wasi.setMemory(memory);
    },
    start() {
      try { exports._initialize?.(); exports.codex_start(); } catch (e) { return fail(e); }
      schedule(0);
    },
    send(message) {
      inbox.push(enc.encode(JSON.stringify(message)));
      wake(0);
    },
    stop() {
      stopped = true;
      clearTimeout(timer);
      for (const [, x] of handles) x.abort?.abort();
      handles.clear();
    },
    get running() { return !stopped; },
  };
}

function toBytes(x) {
  if (x == null) return new Uint8Array();
  return typeof x === "string" ? enc.encode(x) : x instanceof Uint8Array ? x : new Uint8Array(x);
}
function concat(chunks) {
  const out = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
  let o = 0;
  for (const c of chunks) { out.set(c, o); o += c.length; }
  return out;
}

/** Compile the module, wire the host, start Codex. Returns the host. */
export async function startCodex(source, options) {
  const module = source instanceof WebAssembly.Module ? source
    : source instanceof Response ? await WebAssembly.compileStreaming(source)
    : await WebAssembly.compile(source);
  const host = createCodexHost(options);
  const instance = await WebAssembly.instantiate(module, host.imports(module));
  host.attach(instance);
  host.start();
  return host;
}
