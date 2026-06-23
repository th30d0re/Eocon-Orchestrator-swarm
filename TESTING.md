# Testing & verification

## Fast gate (run on every change)

```bash
./scripts/verify.sh
```

Runs, with no special toolchain:
- **orchestrator** — `cargo test` (swarm engine, gist/verifier/watchdog, UI render tests)
- **sidecar** — `swift build` (stub) + `localmodel-lm/scripts/test-protocol.sh`

This deliberately does **not** build the MLX or Core AI backends (they need extra
toolchains — see below). It's the dependency-light gate suitable for all changes.

## Sidecar protocol contract (`localmodel-lm/scripts/test-protocol.sh`)

Black-box test of the JSON-lines wire contract the orchestrator depends on:

| Check | Why it matters |
|-------|----------------|
| response `{session_id,text,tokens,latency_ms}` on stdout | the data channel shape |
| status `{status:...}` on **stderr only** | a status line on stdout corrupts the data channel and breaks the orchestrator's stderr `ready` gate |
| multiple requests → multiple responses | multi-session serving |
| malformed line skipped, valid line still answered | robustness to junk input |
| `coreai` fails loud when unavailable (non-zero exit, error on stderr, no stdout) | the orchestrator must distinguish a dead backend from a silent one |
| unknown backend → stub fallback | safe default |

> Note: `mlx` is the real backend whenever the MLX deps are active (as on `main`), so it is
> *not* asserted fail-loud here — that only holds in a build compiled without the MLX deps.

## Backend builds (need extra toolchains — not in the fast gate)

| Backend | Command | Requires |
|---------|---------|----------|
| MLX (real inference) | `localmodel-lm/scripts/build-mlx.sh` | xcodebuild + Metal Toolchain. Produces `mlx-swift_Cmlx.bundle/.../default.metallib` next to the binary; **plain `swift build` omits the metallib** and MLX dies at runtime. Run the binary with the bundle alongside it. |
| Core AI | `localmodel-lm/scripts/build-coreai.sh` | the **27.0+ SDK** (Xcode-beta) — `DEVELOPER_DIR=/Applications/Xcode-beta.app/Contents/Developer`. The default `xcode-select` (26.x) lacks `CoreAI.framework`, so a normal build uses the fail-loud fallback. |

### Backend smoke test (manual, downloads a model on first run)
```bash
BIN=localmodel-lm/.build/xcode/Build/Products/Debug/localmodel-lm
printf '{"session_id":"s1","prompt":"hello"}\n' \
  | EOCON_MLX_MODEL=mlx-community/Qwen2.5-0.5B-Instruct-4bit "$BIN" mlx
```
Verified end-to-end on `main` (2.x). On the 3.x branch the backend builds + activates but
the HF download adapter stalls — see `NEXT_STEPS.mdx` §6.
