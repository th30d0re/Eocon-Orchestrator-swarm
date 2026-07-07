# Packaging Verification Report — Eocon Agent Runner

**Date:** 2026-07-06  
**Verifier:** packaging sub-swarm  
**Scope:** `orchestrator/deploy/Containerfile.agent-runner` and `orchestrator/deploy/run-eocon-agent-runner.sh`

## 1. What Was Checked

This report verifies the new Eocon-Orchestrator-swarm packaging artifacts against:

1. The tulu reference implementation (paths shown as examples; substitute `$HOME` on your machine):
   - `$HOME/Dev/tulu/external/URL-to-Text/Python_MVP/deploy/Containerfile.video-mcp`
   - `$HOME/Dev/tulu/external/URL-to-Text/Python_MVP/deploy/run-apple-container-video-mcp.sh`
   - `$HOME/Dev/tulu/external/URL-to-Text/Python_MVP/docs/VIDEO_APPLE_CONTAINER_MIGRATION.md`
2. The Eocon-Orchestrator-swarm plan:
   - `$HOME/.kimi-code/sessions/wd_tools_085f72d1d288/session_7f140868-7223-45b6-873f-8f55e1c0abf8/agents/main/plans/quasar-green-arrow-cloak.md`
3. Existing defaults in `orchestrator/src/agents/sandbox.rs`.

## 2. Containerfile.agent-runner

### Requirements from Plan

- Base image: `python:3.11-slim`
- Include `sysvinit-core`, `ffmpeg`, `git`, `curl`, `ca-certificates`, `build-essential`
- Include Rust toolchain via rustup
- Optionally include Swift toolchain
- `WORKDIR /app`
- `ENV PATH` includes cargo bin
- `ENTRYPOINT ["/sbin/init"]` for machine mode
- `CMD ["/bin/bash"]` for run mode
- Comments explaining choices

### Verification

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Base `python:3.11-slim` | ✅ | `FROM python:3.11-slim` |
| `sysvinit-core` installed | ✅ | `apt-get install ... sysvinit-core` |
| `ffmpeg` installed | ✅ | `apt-get install ... ffmpeg` |
| `git` installed | ✅ | `apt-get install ... git` |
| `curl` installed | ✅ | `apt-get install ... curl` |
| `ca-certificates` installed | ✅ | `apt-get install ... ca-certificates` |
| `build-essential` installed | ✅ | `apt-get install ... build-essential` |
| Rust via rustup | ✅ | `curl ... sh.rustup.rs | sh -s -- ...` |
| Optional Swift toolchain | ✅ | `ARG INSTALL_SWIFT=false` with conditional install block |
| `WORKDIR /app` | ✅ | `WORKDIR /app` |
| `ENV PATH` includes cargo | ✅ | `ENV PATH=/root/.cargo/bin:${PATH}` |
| `ENTRYPOINT ["/sbin/init"]` | ✅ | `ENTRYPOINT ["/sbin/init"]` |
| `CMD ["/bin/bash"]` | ✅ | `CMD ["/bin/bash"]` |
| Comments explaining choices | ✅ | Comments on init, packages, rustup, Swift, entrypoint |

### Alignment with Tulu Reference

- Tulu installs `sysvinit-core` so the image can boot as a Container machine; this image preserves that decision.
- Tulu installs `ffmpeg` and `git`; this image keeps both because agent workflows may still need media processing and version control.
- Tulu sets `PYTHONUNBUFFERED=1`, `PYTHONDONTWRITEBYTECODE=1`, and `PYTHONPATH=/app`; this image keeps the same Python environment defaults.
- Tulu copies application source into the image; this image does **not** copy source because agents mount the host project at runtime, matching the plan's design.
- Tulu uses a fixed entrypoint for the MCP server; this image uses `/sbin/init` plus an overridable bash CMD to support both machine mode and ephemeral runs.

### Issues

- None identified.

## 3. run-eocon-agent-runner.sh

### Requirements from Plan

- Subcommands: `build`, `run`, `test`, `config`, `machine-create`, `machine-run`, `machine-test`, `machine-shell`
- `IMAGE_NAME="eocon-agent-runner:latest"`
- `MACHINE_NAME="eocon-pool"`
- `COMMON_RES=(-m 8G)`
- Include env vars for offline mode
- Mirror tulu helper script structure
- Make it executable
- Test with `shellcheck` if available

### Verification

| Requirement | Status | Evidence |
|-------------|--------|----------|
| Subcommand `build` | ✅ | `cmd_build()` |
| Subcommand `run` | ✅ | `cmd_run()` |
| Subcommand `test` | ✅ | `cmd_test()` |
| Subcommand `config` | ✅ | `cmd_config()` |
| Subcommand `machine-create` | ✅ | `cmd_machine_create()` |
| Subcommand `machine-run` | ✅ | `cmd_machine_run()` |
| Subcommand `machine-test` | ✅ | `cmd_machine_test()` |
| Subcommand `machine-shell` | ✅ | `cmd_machine_shell()` |
| `IMAGE_NAME` default | ✅ | `IMAGE_NAME="${IMAGE_NAME:-eocon-agent-runner:latest}"` |
| `MACHINE_NAME` default | ✅ | `MACHINE_NAME="${MACHINE_NAME:-eocon-pool}"` |
| `COMMON_RES=(-m 8G)` | ✅ | `COMMON_RES=("-m" "8G")` |
| Offline-mode env var | ✅ | `EOCON_OFFLINE=${EOCON_OFFLINE:-0}` plus `CARGO_HOME`/`RUSTUP_HOME` |
| Mirrors tulu structure | ✅ | `require_container`, `cmd_*`, `main` dispatch, help text |
| Executable bit | ✅ | `chmod +x` applied; `ls -l` shows `-rwxr-xr-x` |
| `shellcheck` | ⚠️ | Not installed on this host; `bash -n` passes syntax validation |

### Alignment with Tulu Reference

- Same `set -euo pipefail` and `require_container` guard.
- Same `PROJECT_DIR` resolution relative to the script location.
- Same `COMMON_RES` memory default (`-m 8G`).
- Same `container machine create --name <name> --set-default <image>` invocation.
- Same single-quoted command string convention for `container machine run` to preserve shell quoting.
- Same help-text style and environment-variable documentation.
- Differences are deliberate: this script targets general agent commands rather than a video MCP server, so smoke tests check `uname`, `python3`, `cargo`, and `rustc` instead of Whisper-specific imports.

### Issues

- `shellcheck` is not installed on the host used for verification, so static-analysis linting could not be performed. `bash -n` confirms the script is syntactically valid.

## 4. Cross-Reference with Existing Codebase

- `orchestrator/src/agents/sandbox.rs:21-22` already maps `RiskLabel::Safe` and `RiskLabel::Low` to `"eocon-pool"`, matching the `MACHINE_NAME` default in the helper script.
- `orchestrator/src/agents/sandbox.rs:27` sets `memory_gb: 8`, matching `COMMON_RES=(-m 8G)`.
- The placeholder `ContainerMachineRunner` in `sandbox.rs` expects to invoke the `container` CLI with the image and machine names defined here.

## 5. Files from Other Swarms

The plan assigns additional artifacts to the Rust implementation and validation sub-swarms (e.g., `orchestrator/src/agents/sandbox.rs`, `orchestrator/tests/container_runner_test.rs`, `docs/CONTAINER_SANDBOX.md`). At the time of this verification, the Rust source files remain placeholders and the validation tests have not been added, so they are outside the scope of this packaging-only report.

## 6. Readiness Assessment

The packaging artifacts are **ready for the Rust implementation swarm**:

- `Containerfile.agent-runner` satisfies every requirement in the plan and is internally consistent.
- `run-eocon-agent-runner.sh` exposes the required subcommands, uses the required defaults, and follows the proven tulu helper structure.
- Both artifacts align with the existing `SandboxConfig` defaults (`eocon-pool`, 8 GiB memory).
- No blocking issues were found.

### Recommended Next Steps for the Rust Swarm

1. Consume `IMAGE_NAME=eocon-agent-runner:latest` and `MACHINE_NAME=eocon-pool` as defaults in `ContainerMachineRunner`.
2. Use `container machine set --name eocon-pool cpus=4 memory=8G home-mount=ro` to match `SandboxConfig`.
3. Add the `--runner pty|container` CLI flag and wire `ContainerMachineRunner` into `App`/`SwarmEngine`.
4. Run `./deploy/run-eocon-agent-runner.sh build && ./deploy/run-eocon-agent-runner.sh test` as an integration smoke test.
5. Install `shellcheck` and run it on `run-eocon-agent-runner.sh` before merging.

## 7. Sign-Off

Packaging verification completed. The image definition and helper script are consistent with the tulu reference and the Eocon-Orchestrator-swarm plan and can be handed off to the Rust implementation swarm.
