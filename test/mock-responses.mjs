// A stand-in for the Responses API (POST /v1/responses, SSE) with a scripted agent:
// turn 1 runs one shell command through whichever shell tool Codex offers, then answers with
// the command's output; any later request (the tool result coming back) gets a final message.
// Every request body is kept for the test to inspect (`requests`).
import http from "node:http";

export function startMock({ port = 0 } = {}) {
  const requests = [];
  const server = http.createServer(async (req, res) => {
    const chunks = [];
    for await (const c of req) chunks.push(c);
    const raw = Buffer.concat(chunks).toString("utf8");
    let body = {};
    try { body = raw ? JSON.parse(raw) : {}; } catch {}
    requests.push({ method: req.method, url: req.url, headers: req.headers, body });
    if (req.method === "GET" && req.url.startsWith("/v1/models")) {
      res.writeHead(200, { "content-type": "application/json" });
      return res.end(JSON.stringify({ models: [] }));
    }
    if (req.method !== "POST" || !req.url.startsWith("/v1/responses")) {
      res.writeHead(404); return res.end("not here");
    }
    res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache" });
    const send = (event) => res.write(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);
    const id = `resp_${requests.length}`;
    send({ type: "response.created", response: { id } });

    const input = body.input ?? [];
    const toolOutput = input.find((i) => i.type === "function_call_output" || i.type === "custom_tool_call_output");
    const tools = (body.tools ?? []).map((t) => t.name ?? t.type);
    if (!toolOutput) {
      const call = shellCall(tools);
      if (call) {
        const item = { type: "function_call", id: "fc_1", call_id: "call_1", name: call.name, arguments: JSON.stringify(call.args) };
        send({ type: "response.output_item.added", item });
        send({ type: "response.output_item.done", item });
        send({ type: "response.completed", response: { id, usage: usage() } });
        return res.end();
      }
    }
    const said = toolOutput ? `The command printed: ${JSON.stringify(String(toolOutput.output ?? "").slice(0, 400))}` : `No shell tool among: ${tools.join(", ")}`;
    const msg = { type: "message", id: "msg_1", role: "assistant", content: [] };
    send({ type: "response.output_item.added", item: msg });
    for (const word of said.split(/(?<= )/)) send({ type: "response.output_text.delta", item_id: "msg_1", delta: word });
    send({ type: "response.output_item.done", item: { ...msg, content: [{ type: "output_text", text: said }] } });
    send({ type: "response.completed", response: { id, usage: usage() } });
    res.end();
  });
  return new Promise((resolve) => server.listen(port, "127.0.0.1", () => resolve({
    url: `http://127.0.0.1:${server.address().port}/v1`, requests, close: () => server.close(),
  })));
}

const usage = () => ({ input_tokens: 10, input_tokens_details: { cached_tokens: 0 }, output_tokens: 5, output_tokens_details: { reasoning_tokens: 0 }, total_tokens: 15 });

const COMMAND = "echo hello from $(basename $PWD) && ls -a && cat notes.txt";
function shellCall(tools) {
  if (tools.includes("exec_command")) return { name: "exec_command", args: { cmd: COMMAND } };
  if (tools.includes("shell_command")) return { name: "shell_command", args: { command: COMMAND } };
  if (tools.includes("shell")) return { name: "shell", args: { command: ["bash", "-lc", COMMAND] } };
  return null;
}
