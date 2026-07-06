# Rust Implementation Handoff — Apple Container Machine Runner

**Date:** 2026-07-06  
**Scope:** `Eocon-Orchestrator-swarm/orchestrator/src/...`  
**Status:** `cargo check` and `cargo check --tests` pass. `cargo test` was not run per instructions.

## Summary

Implemented the Rust-side integration of the Apple Container Machine runner backend as specified in the approved plan. The PTY runner remains the default; the container backend is opt-in via `--runner container`, config, or environment.

## Files Changed

| File | Change |
|------|--------|
| `src/agents/runner.rs` | Added `pub risk_tier: RiskLabel` to `AgentCmd`; added `machine_name`, `dedicated`, `container_pid` to `RunHandle`; updated `PtyRunner` to initialize new fields to `None`/`false`. |
| `src/agents/sandbox.rs` | Replaced placeholder with real `ContainerMachineRunner` using the Apple `container` CLI; added `resolve_machine`, `build_run_args`, image/machine lifecycle helpers, and inline unit tests. |
| `src/agents/swift.rs` | Added `risk_tier: RiskLabel::Medium` to `build_agent_cmd` and `test_agent_cmd`. |
| `src/swarm/engine.rs` | `SwarmEngine::new` now accepts `Arc<dyn AgentRunner>`; removed hardcoded `PtyRunner`. |
| `src/swarm/agent.rs` | `AgentInstance::pause` now accepts `&dyn AgentRunner` and calls `runner.kill` instead of `handle.child.kill`. |
| `src/swarm/commands.rs` | `/swarm reset` preserves the existing runner when rebuilding the engine. |
| `src/lib.rs` | Added `pub mod config`; added `--runner pty|container` CLI flag; added `runner_backend` to `Config` with CLI > config > env > default resolution. |
| `src/config.rs` | **New** — `RunnerBackend` enum (`clap::ValueEnum` + serde) and `AppConfig`. |
| `src/app.rs` | `App::new` accepts `RunnerBackend`; builds `ContainerMachineRunner` when requested and falls back to `PtyRunner` if the `container` CLI is unavailable; passes runner to `SwarmEngine::new`. |
| `src/main.rs` | `eos serve` now forwards `args.runner` into the `Config`. |
| `src/ui/chat_panel.rs`, `src/ui/mod.rs`, `src/ui/prompt.rs`, `src/ui/workflow_panel.rs` | Updated test `App::new` calls to supply `RunnerBackend::Pty`. |

## Design Notes

- `ContainerMachineRunner` spawns `container machine run` through a local PTY via `portable_pty`, so the existing `RunHandle` PTY fields remain populated and no stub traits are required. Output is streamed through the same `mpsc` channel pattern used by `PtyRunner`.
- Tier resolution uses `SandboxConfig::tier_machine_map`:
  - `Safe`/`Low` → pooled `eocon-pool` machine.
  - `Medium`/`High`/`Critical` → dedicated `eocon-agent-<uuid>` machine.
- Dedicated machines are stopped and deleted on `kill`; cleanup is verified by polling `container machine inspect` until the machine is gone (10 s timeout).
- Image existence is checked with `container image inspect eocon-agent-runner:latest`; if missing, a clear error message points to `./deploy/run-eocon-agent-runner.sh build`.
- The `container` CLI availability check uses `container --version`; if unavailable and `--runner container` was requested, the app falls back to `PtyRunner` with a printed warning.

## Compilation

```text
$ cargo check
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo check --tests
    Finished `dev` profile [unoptimized + debuginfo] target(s)
```

No warnings or errors.

## Known Limitations / TODOs

1. **Writable memory bind-mount:** The plan mentions bind-mounting `.eocon/memory/<agent>/` writable, but the exact Apple `container machine run` volume-mount flag is not yet exercised by the packaging helper. Left as a TODO for the validation swarm.
2. **Config/env override semantics:** Because the CLI flag has `default_value = "pty"`, the code only consults `AppConfig` and `EOCON_RUNNER_BACKEND` when the CLI value resolves to `Pty`. Explicit `--runner pty` can therefore still be overridden by config/env, which may be surprising.
3. **No integration tests yet:** The validation swarm is responsible for `tests/container_runner_test.rs` and running `cargo test`.
4. **No docs update:** Plan-closure docs (`docs/CONTAINER_SANDBOX.md`, `NEXT_STEPS.mdx`, etc.) are assigned to the parent review step.
5. **Shell-out robustness:** Machine creation/inspection uses simple exit-code checks; more nuanced JSON parsing could be added if the CLI output format changes.

## Git Status

- Committed packaging artifacts as `b7d9ded`.
- Committed implementation as `d2237b1`.
- Working tree is clean.
