# Memory Store Handoff — SQLite + JSONL Hybrid

## Summary

Implemented the hybrid memory store for Eocon-Orchestrator-swarm: per-agent
append-only JSONL files for isolation, mirrored to a SQLite database (WAL mode)
for queries and dashboards.

## Files Added / Modified

- `orchestrator/src/memory/mod.rs` — new public module, re-exports
  `MemoryStore`, `AgentMemory`, `MemoryRecord`, `MemoryStoreError`.
- `orchestrator/src/memory/store.rs` — core implementation:
  - `MemoryStore`: SQLite connection + JSONL root, cheaply `Clone` via
    `Arc<Mutex<Inner>>`.
  - `AgentMemory`: per-agent in-memory cache + persistence through
    `MemoryStore`.
  - `MemoryRecord`: row type returned by searches.
  - Methods: `open`, `open_default`, `agent_memory`, `append_message`,
    `get_messages`, `search_messages`, `mirror_jsonl_to_sqlite`.
  - Unit tests for append/get, search, mirror, `AgentMemory` persistence, and
    isolation.
- `orchestrator/src/swarm/memory.rs` — now re-exports from
  `crate::memory::store` and keeps `default_memory_dir` / `project_memory_dir`
  helpers.
- `orchestrator/src/swarm/agent.rs` — `AgentInstance::new` now takes a
  `MemoryStore` and constructs the new `AgentMemory`.
- `orchestrator/src/swarm/engine.rs` — `SwarmEngine` owns a `MemoryStore` and
  clones it into each spawned agent; `spawn_pty_agent` sets `agent_id` on
  `AgentCmd`.
- `orchestrator/src/app.rs` — initializes `MemoryStore` at startup using
  `~/.eocon/memory.sqlite` for SQLite and `<project_dir>/.eocon/memory` for
  JSONL.
- `orchestrator/src/swarm/commands.rs` — `/swarm reset` preserves the running
  `MemoryStore`.
- `orchestrator/src/agents/runner.rs` — `AgentCmd` gained an optional
  `agent_id` field so the container runner can expose per-agent memory.
- `orchestrator/src/agents/sandbox.rs` — `build_run_args` exposes the absolute
  per-agent memory directory via `EOCON_AGENT_MEMORY_DIR`. Default
  `home_mount_readonly` changed to `false` so the guest can write back through
  the home mount. Added a test verifying the env var.
- `orchestrator/src/ui/agent_card.rs` — tests updated to provide a temp
  `MemoryStore`.
- `orchestrator/tests/container_runner_test.rs` — `AgentCmd` literals updated
  with `agent_id`.
- `orchestrator/tests/memory_store_test.rs` — new integration tests covering
  round-trip, search, mirror, `AgentMemory` load, and isolation.

## Design Notes

- **Source of truth:** JSONL. Every `append_message` writes to
  `<jsonl_root>/<agent_id>/messages.jsonl` and then inserts the same row into
  SQLite.
- **Query surface:** SQLite. `get_messages` and `search_messages` read from the
  mirror table.
- **Recovery / bulk sync:** `mirror_jsonl_to_sqlite` re-reads the JSONL file
  and replaces the SQLite rows for that agent.
- **Isolation:** Each agent has its own JSONL directory and SQLite rows are
  scoped by `agent_id`. `AgentMemory` caches only its own messages.
- **Sharing:** `MemoryStore` uses `Arc<Mutex<Inner>>` so the global store and
  every per-agent `AgentMemory` share one SQLite connection without lifetime
  complexity.

## Container Bind-Mount Caveat

Apple's `container machine run` CLI does not expose a per-directory bind-mount
flag; only `--home-mount` (configured at machine creation time) is available.
Consequently, `build_run_args` currently passes the absolute memory directory
path via the `EOCON_AGENT_MEMORY_DIR` environment variable and leaves the home
mount read-write (`home_mount_readonly: false` by default). When Apple adds a
native volume-mount flag, the env-var block can be replaced with a true
bind-mount such as `--volume <host_path>:<guest_path>`.

## Test Results

```text
$ cargo test
running 71 tests  ... ok (lib)
running 2 tests   ... ok (container_runner_test)
running 1 test    ... ok (control_test)
running 1 test    ... ok (integration_test)
running 5 tests   ... ok (memory_store_test)
running 1 test    ... ok (doc-tests)
```

All tests pass.

## TODOs / Follow-ups

1. Replace the `EOCON_AGENT_MEMORY_DIR` env-var workaround with a native
   `container machine run` bind-mount flag once Apple exposes one.
2. Consider FTS5 in SQLite for full-text search instead of `LIKE`.
3. Add a background or periodic mirror task for very high append rates.
4. Expose memory queries to the dashboard via a control-plane endpoint.
5. Add migration/import of legacy per-agent JSONL files into SQLite on first
   open.
6. Add memory-operation metrics (append count, query latency, mirror count).
