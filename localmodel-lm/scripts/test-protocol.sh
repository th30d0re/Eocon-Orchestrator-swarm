#!/usr/bin/env bash
# Black-box contract test for the localmodel-lm sidecar JSON-lines protocol.
#
# Exercises the `stub` backend (no ML deps needed at runtime) and asserts the wire
# contract that the Rust orchestrator relies on:
#   - request {session_id,prompt,role?} on stdin -> response {session_id,text,tokens,latency_ms} on stdout
#   - status {status:preparing|ready,...} goes to STDERR only (never stdout — that would
#     corrupt the data channel and break the orchestrator's stderr `ready` gate)
#   - multiple requests (multiple sessions) each get a response
#   - malformed input lines are skipped without crashing
#
# Usage: ./scripts/test-protocol.sh
# Exit 0 = all pass. Builds the stub with `swift build` if needed.
set -uo pipefail

cd "$(dirname "$0")/.."

PASS=0
FAIL=0
ok()   { echo "  ok  - $1"; PASS=$((PASS+1)); }
bad()  { echo "  FAIL- $1"; FAIL=$((FAIL+1)); }

echo "==> building stub (swift build)"
if ! swift build >/tmp/proto_build.log 2>&1; then
    echo "BUILD FAILED — see /tmp/proto_build.log"; tail -20 /tmp/proto_build.log; exit 1
fi
BIN=./.build/debug/localmodel-lm
[ -x "$BIN" ] || { echo "binary not found at $BIN"; exit 1; }

# --- Test 1: single request -> well-formed response on stdout ---------------
OUT=$(printf '{"session_id":"s1","prompt":"hello world"}\n' | "$BIN" stub 2>/dev/null)
echo "$OUT" | python3 -c '
import sys, json
line = sys.stdin.read().strip()
assert line, "no stdout"
o = json.loads(line)  # must be valid JSON
for k in ("session_id","text","tokens","latency_ms"):
    assert k in o, f"missing field {k}"
assert o["session_id"] == "s1", "session_id not echoed"
assert isinstance(o["tokens"], int), "tokens not int"
' && ok "response is well-formed JSON with required fields" || bad "response shape"

# --- Test 2: status on stderr, NOT stdout (channel separation regression) ---
ERR=$(printf '{"session_id":"s1","prompt":"hi"}\n' | "$BIN" stub 2>&1 1>/dev/null)
echo "$ERR" | grep -q '"status":"ready"' && ok "ready status present on stderr" || bad "no ready on stderr"
STDOUT_ONLY=$(printf '{"session_id":"s1","prompt":"hi"}\n' | "$BIN" stub 2>/dev/null)
if echo "$STDOUT_ONLY" | grep -q '"status":'; then bad "status leaked into stdout"; else ok "no status on stdout (clean data channel)"; fi

# --- Test 3: two requests -> two responses ----------------------------------
COUNT=$(printf '{"session_id":"a","prompt":"x"}\n{"session_id":"b","prompt":"y"}\n' | "$BIN" stub 2>/dev/null | grep -c '"text"')
[ "$COUNT" -eq 2 ] && ok "two requests -> two responses" || bad "expected 2 responses, got $COUNT"

# --- Test 4: malformed line is skipped, valid line still answered -----------
GOOD=$(printf 'not json at all\n{"session_id":"c","prompt":"z"}\n' | "$BIN" stub 2>/dev/null | grep -c '"session_id":"c"')
[ "$GOOD" -eq 1 ] && ok "malformed line skipped; valid line answered" || bad "malformed-input handling"

echo
echo "==> protocol contract: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
