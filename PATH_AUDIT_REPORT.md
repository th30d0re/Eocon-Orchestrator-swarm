# Path Audit Report — Eocon-Orchestrator-swarm

**Date:** 2026-07-06  
**Scope:** Replace hardcoded `/Users/emmanuel/...` absolute paths with portable alternatives.

## Summary

- **Paths changed:** 29
- **Files changed:** 10
- **Paths left untouched:** Build artifacts in `.venv*/`, `orchestrator/target/`, and `localmodel-lm/.build/` (generated/virtual-env outputs).

## Changes

| File | Original path | Replacement | Reasoning |
|------|---------------|-------------|-----------|
| `orchestrator/src/chat.rs` | `/Users/emmanuel/.kimi-code/bin/kimi` | `home_path(".kimi-code/bin/kimi")` | User home binary; resolve via `dirs::home_dir()`. |
| `orchestrator/src/chat.rs` | `/Users/emmanuel/.bun/bin/codex` | `home_path(".bun/bin/codex")` | User home binary; resolve via `dirs::home_dir()`. |
| `orchestrator/src/chat.rs` | `/Users/emmanuel/.local/bin/cursor` | `home_path(".local/bin/cursor")` | User home binary; resolve via `dirs::home_dir()`. |
| `orchestrator/src/chat.rs` | `/Users/emmanuel/Dev/Tools/Eocon-Orchestrator-swarm/localmodel-lm/.build/...` (3 paths) | `root.join("localmodel-lm/.build/...")` | Project paths; resolve via `EOCON_SWARM_ROOT` env var or fall back to `~/Dev/Tools/Eocon-Orchestrator-swarm`. |
| `orchestrator/src/model_registry.rs` | `/Users/emmanuel/Dev/Tools/Eocon-Orchestrator-swarm/RootLedger-Gemma3-12B-CoreAI/...` | `default_coreai_model_path()` | Project model path; override with `EOCON_COREAI_MODEL`, default computed from home dir. |
| `orchestrator/src/model_registry.rs` | `/Users/emmanuel/.eocon/traces/sessions.jsonl` | `default_traces_sessions_path()` | User data path; override with `EOCON_TRACES_SESSIONS`, default computed from home dir. |
| `scripts/convert-coreai.sh` | `/Users/emmanuel/Documents/Theory/Redefining_racism/training/...` | `${HOME}/Documents/...` | Script default; use `$HOME` and allow `SRC` override. |
| `scripts/convert-coreai.sh` | `/Users/emmanuel/Dev/Tools/Eocon-Orchestrator-swarm` | `${EOCON_SWARM_ROOT:-${HOME}/Dev/Tools/Eocon-Orchestrator-swarm}` | Output dir default; support `EOCON_SWARM_ROOT` env var. |
| `scripts/extract_replay_sessions.py` | `/Users/emmanuel/Dev/Tools/Complete-FABLE.5-traces-2M/data/train.parquet` | `os.environ.get("EOCON_TRACES_PARQUET", Path.home()/...)` | Default dataset path; override with `EOCON_TRACES_PARQUET`. |
| `scripts/dequantize_mlx_model.py` (docstring) | `/Users/emmanuel/Documents/Theory/Redefining_racism/training/...` | `$HOME/Documents/...` | Documentation example only; mark as example. |
| `orchestrator/deploy/PACKAGING_VERIFICATION.md` | `/Users/emmanuel/Dev/tulu/...` and `/Users/emmanuel/.kimi-code/...` | `$HOME/Dev/tulu/...` and `$HOME/.kimi-code/...` | Documentation references; paths shown as examples. |
| `NEXT_STEPS.mdx` | `/Users/emmanuel/Dev/Tools/Eocon-Orchestrator-swarm` | `EOCON_SWARM_ROOT` / directory containing file | Repo root reference; use env var / relative resolution. |
| `NEXT_STEPS.mdx` | `/Users/emmanuel/Dev/Apps/ChAi/ChAi_3` | `$HOME/Dev/Apps/ChAi/ChAi_3` | Documentation example. |
| `NEXT_STEPS.mdx` | `/Users/emmanuel/Documents/Theory/Redefining_racism/training/fused_models/` | `$HOME/Documents/Theory/Redefining_racism/training/fused_models/` | Documentation example. |
| `NEXT_STEPS.mdx` | `/Users/emmanuel/Dev/Tools/Eocon-Orchestrator-swarm/RootLedger-Gemma3-12B-CoreAI/...` | `${EOCON_SWARM_ROOT:-$HOME/Dev/Tools/Eocon-Orchestrator-swarm}/...` | Shell example; use env var. |
| `plans/eocon_swarm_private_memory_e9b065ff.plan.mdx` | `/Users/emmanuel/Dev/Apps/ChAi/ChAi_3` | `$HOME/Dev/Apps/ChAi/ChAi_3` | Documentation example. |
| `plans/port_custom_mcp_to_apple_containerization.plan.mdx` | `/Users/emmanuel/Dev/tulu/...` | `$HOME/Dev/tulu/...` | Documentation example. |

## New/Updated Environment Variables

- `EOCON_SWARM_ROOT` — root of the Eocon-Orchestrator-swarm checkout.
- `EOCON_COREAI_MODEL` — path to the Core AI `.aimodel` bundle.
- `EOCON_TRACES_SESSIONS` — path to the trace-replay JSONL file.
- `EOCON_TRACES_PARQUET` — path to the FABLE.5 traces Parquet file.

## Verification

- `cargo check` in `orchestrator/` passes.
- `bash -n scripts/convert-coreai.sh` passes.
- `python3 -m py_compile scripts/dequantize_mlx_model.py` passes.
- `python3 -m py_compile scripts/extract_replay_sessions.py` passes.

## Paths Not Replaced

Generated/virtual-environment files under `.venv*/`, `orchestrator/target/`, and `localmodel-lm/.build/` contain hardcoded absolute paths. These are build artifacts and are not tracked source; they were left untouched.
