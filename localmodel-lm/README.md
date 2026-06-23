# localmodel-lm

SwiftPM sidecar for on-device inference in Eocon Swarm.

## Backends

- `coreai` — Apple Core AI (`.aimodel` + `InferenceFunction` + states).
- `mlx` — MLX Swift (`mlx-swift-lm` 3.x, Hugging Face weights, wired memory).

## Build

```bash
cd Eocon-Orchestrator-swarm/localmodel-lm
swift build
```

> Note: real MLX/Core AI builds require Xcode and the Metal Toolchain for shader compilation. This v1 skeleton compiles without external dependencies.

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
.localmodel-lm --backend coreai
```
