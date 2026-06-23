#!/usr/bin/env bash
# One-command verification for the whole repo.
#   - orchestrator: cargo test (Rust swarm engine + UI render tests)
#   - sidecar:      swift build (stub) + JSON-lines protocol contract test
#
# Does NOT build the MLX/Core AI backends (those need xcodebuild + the Metal Toolchain /
# 27.0 SDK — see localmodel-lm/scripts/build-mlx.sh and build-coreai.sh). This script is the
# fast, dependency-light gate suitable for every change.
#
# Usage: ./scripts/verify.sh    (run from anywhere; resolves repo root itself)
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FAILED=0
section() { printf '\n=== %s ===\n' "$1"; }
report()  { if [ "$1" -eq 0 ]; then echo "PASS: $2"; else echo "FAIL: $2"; FAILED=1; fi; }

section "orchestrator — cargo test"
( cd "$ROOT/orchestrator" && cargo test --quiet )
report $? "orchestrator cargo test"

section "sidecar — protocol contract test (builds stub + checks JSON-lines wire contract)"
( cd "$ROOT/localmodel-lm" && ./scripts/test-protocol.sh )
report $? "sidecar protocol contract"

section "summary"
if [ "$FAILED" -eq 0 ]; then
    echo "ALL GREEN"
else
    echo "SOME CHECKS FAILED (see above)"
fi
exit "$FAILED"
