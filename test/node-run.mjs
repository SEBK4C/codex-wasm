// End to end in Node: Codex's module on the tab host (host/), a memfs volume, just-bash for
// commands, and the scripted mock Responses API. One turn: Codex calls its shell tool, the
// command runs in just-bash on the volume, the output goes back to the model, the answer
// comes out as events.
//   node test/node-run.mjs path/to/codex_wasm.wasm
import fs from "node:fs";
import { Volume, createFsFromVolume } from "memfs";
import { Bash } from "just-bash";
import { startCodex } from "../host/codex-host.mjs";
import { startMock } from "./mock-responses.mjs";
import { bashAdapter } from "./bash-fs.mjs";

const wasmPath = process.argv[2] ?? "codex_wasm.wasm";
const mock = await startMock();
const HOME = "/home/user", PROJECT = `${HOME}/project`;
// MODEL=<slug>: one of the models Codex ships, from its catalog (code_mode_only relaxed to
// code_mode, as the tab does: no code-mode host here).
const MODEL = process.env.MODEL ?? "mock-model";
const catalog = JSON.parse(fs.readFileSync(new URL("../codex/codex-rs/models-manager/models.json", import.meta.url).pathname.replace("/js/codex/", "/js/codex/"), "utf8"));
for (const m of catalog.models) if (m.tool_mode === "code_mode_only") m.tool_mode = "code_mode";
const vol = Volume.fromJSON({
  [`${HOME}/.codex/models.json`]: JSON.stringify(catalog),
  [`${PROJECT}/notes.txt`]: "a note in the project\n",
  "/tmp/.keep": "",
  [`${HOME}/.codex/config.toml`]: `model = "${MODEL}"
model_provider = "mock"
model_catalog_json = "${HOME}/.codex/models.json"

[model_providers.mock]
name = "mock"
base_url = "${mock.url}"
wire_api = "responses"
`,
});
const vfs = createFsFromVolume(vol);
const bash = new Bash({ fs: bashAdapter(vfs), cwd: PROJECT, env: { HOME, PATH: "/usr/bin:/bin" } });

const shells = new Set(["bash", "sh", "zsh", "/bin/bash", "/bin/sh", "/usr/bin/bash", "/bin/zsh"]);
const quote = (a) => (/^[\w@%+=:,./-]+$/.test(a) ? a : `'${a.replace(/'/g, `'\\''`)}'`);
async function spawn(spec, { stdin }) {
  const args = spec.args ?? [];
  let script;
  const c = args.findIndex((a) => a === "-c" || a === "-lc" || a === "-ic");
  if (shells.has(spec.program) && c >= 0) script = args[c + 1] ?? "";
  else script = [spec.program, ...args].map(quote).join(" ");
  console.log(`[spawn] ${spec.program} ${JSON.stringify(args)} (cwd ${spec.cwd})`);
  const r = await bash.exec(script, { cwd: spec.cwd, env: spec.env, stdin: new TextDecoder().decode(stdin) });
  return { stdout: r.stdout, stderr: r.stderr, exitCode: r.exitCode };
}

const events = [];
let done;
const finished = new Promise((r) => { done = r; });
const host = await startCodex(fs.readFileSync(wasmPath), {
  fs: vfs,
  env: { HOME, CODEX_HOME: `${HOME}/.codex`, PWD: PROJECT, PATH: "/usr/bin:/bin", USER: "user", SHELL: "/bin/bash", TMPDIR: "/tmp", RUST_LOG: process.env.RUST_LOG ?? "warn" },
  spawn,
  fetch: globalThis.fetch,
  log: (line) => process.stderr.write(`[codex] ${line}`),
  onMessage(m) {
    if (m.type === "event") {
      const msg = m.event.msg;
      events.push(msg);
      const brief = msg.type === "agent_message_delta" || msg.type === "agent_message_content_delta" ? JSON.stringify(msg.delta) : "";
      console.log(`[event] ${msg.type} ${brief}`);
      if (msg.type === "turn_complete" || msg.type === "task_complete") done("complete");
      if (msg.type === "error") console.log(JSON.stringify(msg));
    } else {
      console.log(`[${m.type}]`, JSON.stringify(m).slice(0, 400));
      if (m.type === "ready") host.send({ type: "user", text: "Run a command and tell me what it printed." });
      if (m.type === "crash" || m.type === "exit" || m.type === "panic") done(m.type);
    }
  },
});

const timeout = setTimeout(() => done("timeout"), 120_000);
const how = await finished;
clearTimeout(timeout);
host.stop();
mock.close();
const final = events.filter((e) => e.type === "agent_message").map((e) => e.message).join("\n");
console.log(`\n=== ${how}; ${mock.requests.length} model requests; final message: ${final || "(none)"}`);
console.log(`tools offered: ${(mock.requests.find((r) => r.body?.tools)?.body.tools ?? []).map((t) => t.name ?? t.type).join(", ")}`);
process.exit(how === "complete" && /hello from project/.test(final) ? 0 : 1);
