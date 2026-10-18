#!/usr/bin/env bash
# Sync, run a keep-going check on waffle with JSON diagnostics, fetch them to out/check.json,
# and print a summary. Args go to cargo (default: -p codex-thread-manager-sample).
set -euo pipefail
cd "$(dirname "$0")/.."
build/sync.sh
mkdir -p out
args=("$@"); [ ${#args[@]} -eq 0 ] && args=(-p codex-thread-manager-sample)
ssh waffle "cd /root/codex-wasm && build/cargo.sh check --keep-going --message-format=json ${args[*]} > check.json 2> check.err; true"
scp -q waffle:/root/codex-wasm/check.json out/check.json
scp -q waffle:/root/codex-wasm/check.err out/check.err
python3 - <<'PY'
import json
errs=[]
for l in open('out/check.json'):
    try: m=json.loads(l)
    except ValueError: continue
    if m.get('reason')=='compiler-message' and m['message']['level']=='error':
        sp=[s for s in m['message']['spans'] if s.get('is_primary')]
        loc=f"{sp[0]['file_name']}:{sp[0]['line_start']}" if sp else ''
        errs.append((m['package_id'].split('#')[-1].split('/')[-1], loc, m['message']['message']))
for e in errs: print(' | '.join(e))
print(len(errs), 'errors')
PY
grep -E "^error: (failed|could not)" out/check.err | grep -v "could not compile" | head || true
