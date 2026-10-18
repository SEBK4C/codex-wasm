#!/usr/bin/env python3
"""List the `#[cfg(unix)]` gates in codex sources that rustc blamed for a missing item.

Reads cargo's `--message-format=json` output on stdin. For every "found an item that was
configured out" note whose gate (the `cfg(...)` span) is in a codex source file, prints
`file:line: <gate text>` once. With --apply, rewrites a bare `#[cfg(unix)]` on that line to
`#[cfg(any(unix, target_os = "wasi"))]` (other gates are only listed, for a hand fix).
"""
import json
import pathlib
import re
import sys

apply = "--apply" in sys.argv
root = pathlib.Path(sys.argv[sys.argv.index("--root") + 1]) if "--root" in sys.argv else None
seen = {}


def walk(diag):
    for child in diag.get("children", []):
        for span in child.get("spans", []):
            label = span.get("label") or ""
            if "gated here" in label or "gated behind" in label:
                f = span["file_name"]
                if "/registry/" in f or "/git/checkouts/" in f or "/crates/" in f:
                    continue
                text = span.get("text", [{}])[0].get("text", "").strip()
                seen.setdefault((f, span["line_start"]), text)
        walk(child)


for line in sys.stdin:
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    if msg.get("reason") == "compiler-message":
        walk(msg["message"])

for (f, line), text in sorted(seen.items()):
    print(f"{f}:{line}: {text}")
    if apply and root is not None:
        path = root / f
        lines = path.read_text().split("\n")
        old = lines[line - 1]
        new = re.sub(r"#\[cfg\(unix\)\]", '#[cfg(any(unix, target_os = "wasi"))]', old)
        if new != old:
            lines[line - 1] = new
            path.write_text("\n".join(lines))
            print(f"  -> {new.strip()}")
