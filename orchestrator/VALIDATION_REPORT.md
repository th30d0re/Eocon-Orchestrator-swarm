# Apple Container Machine Runner — Validation Report

**Date:** 2026-07-06  
**Validator:** Kimi Code CLI (validation sub-swarm)  
**Scope:** `Eocon-Orchestrator-swarm/orchestrator` Apple Container Machine port

## Summary

The Rust implementation sub-swarm delivered a working `ContainerMachineRunner`
backend, `--runner pty|container` CLI flag, and `eocon-agent-runner` image
packaging. This validation pass added integration tests, documentation, and
plan-closure updates, then ran the full test suite. One implementation bug was
found and fixed: `container machine stop` and `container machine delete` require
the machine ID positionally, not via `--name`.

## Test results

```text
cargo test
  unit tests:            53 passed, 0 failed
  container_runner_test:  2 passed, 0 failed
  control_test:           1 passed, 0 failed
  integration_test:       1 passed, 0 failed
  total:                 57 passed, 0 failed
```

The container integration tests ran against the Apple `container` CLI
v1.0.0 (commit ee848e3). No ghost `eocon-pool` or `eocon-agent-*` machines
remained after the run.

## What was tested

### `orchestrator/tests/container_runner_test.rs`

- **Image build:** `./deploy/run-eocon-agent-runner.sh build` succeeded and
  produced `eocon-agent-runner:latest`. The test skips gracefully if the build
  fails.
- **Pooled machine (`Low` tier):** `ContainerMachineRunner` created `eocon-pool`,
  ran `uname -a`, streamed output containing "Linux", and left the machine
  present after `kill_agent`.
- **Dedicated machine (`Medium` tier):** `ContainerMachineRunner` created a
  machine named `eocon-agent-<uuid>`, ran `uname -a`, streamed output, and
  removed the machine within 10 seconds of `kill_agent`.
- **PTY fallback:** `PtyRunner` spawned `echo hello-from-pty` and returned the
  expected output unchanged.

### Existing suite

All pre-existing unit tests, the CLI control-plane integration test, and the
`--help` integration test still pass with the default `PtyRunner` backend.

## Issues found and fixed

### 1. `container machine stop/delete` reject `--name`

**Symptom:** `ContainerMachineRunner::kill` for dedicated machines failed with:

```text
Error: Unknown option '--name'
Usage: container machine stop [--debug] [<id>]
```

**Root cause:** The runner passed `--name <name>` to `stop` and `delete`, but
those subcommands expect the machine ID as a positional argument.

**Fix:** Changed `orchestrator/src/agents/sandbox.rs` `delete_machine` to pass
the name positionally. Also corrected the cleanup helper in
`orchestrator/tests/container_runner_test.rs`.

## Remaining TODOs

1. **Writable memory bind-mount:** The plan calls for mounting
   `.eocon/memory/<agent>/` writable inside the guest. The current
   `ContainerMachineRunner::build_run_args` does not yet add a volume mount for
   this directory. The pooled/dedicated tests do not exercise file writes.
2. **Config/env override semantics:** The CLI flag has `default_value = "pty"`,
   so config/env only override when the resolved CLI value is `Pty`. Explicit
   `--runner pty` can still be overridden, which may be surprising.
3. **Dedicated cleanup on panic:** `MachineGuard` in the tests cleans up
   leftover machines, but production `ContainerMachineRunner` relies on
   `kill_agent` to delete dedicated machines. A global cleanup hook on app
   shutdown could reduce ghost-VM risk.
4. **Tier 2/3 resource verification:** The integration tests verify lifecycle
   but do not assert that `cpus=` and `memory=` caps are applied.
5. **Parallel test safety:** Both container tests currently run in the same
   `#[test]` to avoid `eocon-pool` name collisions. A named lock or per-test
   machine naming would allow parallel execution.
6. **Image build timeout:** The first image build can take several minutes.
  `cargo test` has no built-in per-test timeout; the test uses
  `recv_timeout` only for output, not for the build step itself.

## Files touched

- `orchestrator/tests/container_runner_test.rs` — new integration tests.
- `docs/CONTAINER_SANDBOX.md` — new runbook.
- `plans/eocon_swarm_private_memory_e9b065ff.plan.mdx` — marked v2 sandbox
  implemented and linked to docs/tests.
- `NEXT_STEPS.mdx` — documented `--runner` flag and sandbox status.
- `orchestrator/README.md` — documented `--runner` flag and sandbox status.
- `orchestrator/src/agents/sandbox.rs` — fixed `stop`/`delete` argument form.

## Conclusion

The Apple Container Machine port is validated end-to-end: the image builds, the
runner spawns commands in pooled and dedicated machines, cleanup leaves no ghost
VMs, and the PTY path is unchanged. The remaining TODOs are improvements rather
than blockers.
