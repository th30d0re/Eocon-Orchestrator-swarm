# Unified LLM Client — Phase 1 Handoff

## Status
Phase 1 complete: stable design document, working Rust core with Anthropic and OpenAI-compatible providers, Python subprocess bridge, and Foundation-compatible shell wrapper.

Cross-repo integration (Phase 2) is NOT in scope for this handoff.

## What Was Delivered

### 1. Design Document
- **Path:** `/Users/emmanuel/Dev/Tools/cross-audit-site/artifacts/UNIFIED_LLM_CLIENT_DESIGN.md`
- Covers core request/response/streaming/tool-call abstraction, supported providers, language bindings, TOML/JSON config, error handling, and migration path from existing provider layers.

### 2. Rust Core (`Eocon-Orchestrator-swarm/orchestrator/src/llm/`)
- `mod.rs` — module entry point and public re-exports.
- `client.rs` — `LlmClient` trait, `Request`/`Response`/`Message`/`ToolCall` types, `UnifiedLlmClient`, builder helpers, iterator-based streaming.
- `provider.rs` — `Provider` trait, `AnthropicProvider`, `OpenAiCompatibleProvider` (reused for OpenAI, DeepSeek, LM Studio, etc.), and provider-agnostic text tool-call parsing inspired by ORBOS `src/harness/engine.py`.
- `config.rs` — TOML/JSON configuration loading, env-var API key resolution, provider defaults.

### 3. CLI Integration
- New `eos llm complete` and `eos llm stream` subcommands in `src/lib.rs` / `src/main.rs`.
- Reads a JSON `Request` from stdin and emits a JSON `Response` (complete) or JSON-lines `StreamEvent` (stream).

### 4. Python Bindings Scaffold
- **Path:** `orchestrator/python/llm_client.py`
- Subprocess/JSON-lines bridge that spawns `eos llm complete|stream`.

### 5. Shell Wrapper
- **Path:** `orchestrator/deploy/llm-request.sh`
- Foundation-compatible interface: `llm-request.sh <provider> <model> <max_tokens> < prompt.txt`

## File Inventory

```
cross-audit-site/artifacts/UNIFIED_LLM_CLIENT_DESIGN.md
Eocon-Orchestrator-swarm/orchestrator/src/llm/mod.rs
Eocon-Orchestrator-swarm/orchestrator/src/llm/client.rs
Eocon-Orchestrator-swarm/orchestrator/src/llm/provider.rs
Eocon-Orchestrator-swarm/orchestrator/src/llm/config.rs
Eocon-Orchestrator-swarm/orchestrator/python/llm_client.py
Eocon-Orchestrator-swarm/orchestrator/deploy/llm-request.sh
Eocon-Orchestrator-swarm/orchestrator/LLM_CLIENT_HANDOFF.md
```

## Verification

```bash
cd /Users/emmanuel/Dev/Tools/Eocon-Orchestrator-swarm/orchestrator
cargo check
# => clean (1 dead_code warning in pre-existing memory/store.rs)

cargo test --lib
# => 70 passed; 0 failed; 0 ignored

cargo test llm
# => 12 passed; 0 failed (LLM module tests only)
```

Full `cargo test` (including container runner integration tests) passes when run per-test-crate; the combined suite can exceed 5 minutes due to container lifecycle tests and is not recommended for routine verification.

## Known Issues / Notes

1. **Streaming is buffered in Phase 1.** `Provider::stream()` currently returns the complete response as a single `StreamEvent::Content` chunk. The trait interface is stable; a future SSE implementation can replace the internal reader without changing callers.
2. **Tool parsing is text-only.** Native provider tool schemas are not yet sent in request payloads. The parser merges text-harness calls with any native `tool_use`/`tool_calls` returned by providers.
3. **Repo had pre-existing uncommitted changes.** The orchestrator working tree contained an in-progress memory store / agent integration that did not compile. The commits for this work include incidental build fixes required to make `cargo check` and `cargo test` green. Those fixes are limited to:
   - `src/memory/store.rs` — error conversion in `search_messages`, mutable borrow in `mirror_jsonl_to_sqlite`, test expectation correction.
   - `src/agents/swift.rs` — add missing `agent_id` field to `AgentCmd` initializers.
   - `src/ui/agent_card.rs` — pass `MemoryStore` to test agent constructor.
4. **Google/local providers not yet implemented.** They are designed and listed in the design doc; Phase 2 will add `GoogleProvider`, `OllamaProvider`, and reuse `OpenAiCompatibleProvider` for LM Studio / vLLM.

## Phase 2 Checklist

- [ ] Implement true SSE streaming over `ureq::Response::into_reader()`.
- [ ] Add `GoogleProvider`.
- [ ] Add native tool schemas to Anthropic/OpenAI requests.
- [ ] Add `OllamaProvider` / LM Studio reuse of `OpenAiCompatibleProvider`.
- [ ] Migrate `src/chat.rs` CLI-spawning paths to `UnifiedLlmClient` where possible.
- [ ] Add optional PyO3 feature for native Python module.
- [ ] Update Foundation-V1 `ai_provider.sh` to call `deploy/llm-request.sh`.
- [ ] Share tool-parser test vectors between ORBOS Python and Rust implementations.
