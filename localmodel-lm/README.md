# localmodel-lm

SwiftPM sidecar for on-device inference in Eocon Swarm.

## Backends

- `coreai` — Apple Core AI (`.aimodel` + `InferenceFunction` + states).
- `mlx` — MLX Swift (`mlx-swift-lm` 3.x on this branch; the verified 2.x baseline lives on `main`).

## Production backend decision

**Default to MLX 2.x for production.** The `main` branch pins `mlx-swift` 0.29.x / `mlx-swift-lm` 2.29.x and has been verified end-to-end with `mlx-community/Qwen2.5-0.5B-Instruct-4bit`.

This branch carries the **3.x migration** (`mlx-swift` 0.31.x / `mlx-swift-lm` 3.31.x). The HF downloader adapter stalls at ~4%, so 3.x is **not production-ready** until the local-dir load path below is validated on real weights.

## Build

```bash
cd Eocon-Orchestrator-swarm/localmodel-lm
swift build
```

> Note: real MLX/Core AI builds require Xcode and the Metal Toolchain for shader compilation. This v1 skeleton compiles without external dependencies.

### MLX (real on-device inference)

```bash
./scripts/build-mlx.sh
BIN=.build/xcode/Build/Products/Debug/localmodel-lm
printf '{"session_id":"s1","prompt":"hello"}\n' \
  | EOCON_MLX_MODEL=mlx-community/Qwen2.5-0.5B-Instruct-4bit "$BIN" mlx
```

### MLX local-directory load (3.x validation path)

Pre-download weights with the 2.x path or `huggingface-cli`, then point at the local directory:

```bash
EOCON_MLX_LOCAL_DIR=/path/to/mlx-community.Qwen2.5-0.5B-Instruct-4bit \
  ./.build/xcode/Build/Products/Debug/localmodel-lm mlx
```

This bypasses the broken `swift-hf-api` 0.2.x `HubClient` downloader and validates the 3.x `ChatSession` + `KVCache` + wired-memory path.

### Core AI (macOS 27.0 SDK)

```bash
./scripts/build-coreai.sh
BIN=.build/xcode-coreai/Build/Products/Debug/localmodel-lm
printf '{"session_id":"s1","prompt":"hello"}\n' \
  | EOCON_COREAI_MODEL=/path/to/model.aimodel "$BIN" coreai
```

Requires Xcode-beta with the macOS 27.0 SDK and the Metal Toolchain.

## Protocol

JSON-lines over stdin/stdout:

```json
// request
{"session_id":"a1","prompt":"...","role":"reasoning"}

// response
{"session_id":"a1","text":"...","tokens":8,"latency_ms":1}

// status on stderr
{"status":"preparing","phase":"specialization"}
{"status":"ready","backend":"coreai","models":["..."]}
```

## Run

```bash
./.build/debug/localmodel-lm --backend stub
./.build/debug/localmodel-lm --backend coreai
./.build/debug/localmodel-lm --backend mlx
```
