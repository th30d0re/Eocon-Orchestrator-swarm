# Eocon Orchestrator Swarm

[![CI](https://github.com/th30d0re/Eocon-Orchestrator-swarm/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/th30d0re/Eocon-Orchestrator-swarm/actions/workflows/ci.yml)

A Rust terminal-UI multi-agent swarm orchestrator with isolated private memory and verified shared gists. Each specialist agent keeps its own reasoning and tool history; only compact, evidence-backed gists are admitted to the shared board after verification. The orchestrator supports host PTY execution and Apple Container Machine sandboxing, and pairs with an on-device Swift sidecar for local MLX and Core AI inference.

This repository is a standalone project. Do not re-initialize git anywhere above this directory.

## Key features

- **Private-memory swarm**: every agent owns an isolated `AgentMemory` (chat history, system prompt, raw logs). Other agents cannot read it.
- **Gist board**: the only shared context layer. Agents publish compressed, verified claims with cited evidence; unsupported claims are rejected.
- **PACT policy engine**: hierarchical safety — immutable global policy plus configurable user policy. Every action is classified as `Comply`, `Guide`, or `Reject`.
- **Container sandboxing**: agents can run inside Apple Container Machines with tier-based isolation. Low-risk agents share a pooled machine; higher-risk agents get dedicated machines that are destroyed on kill.
- **On-device inference sidecar**: `localmodel-lm` provides a JSON-lines stdin/stdout protocol for MLX and Core AI backends, served from the macOS host.
- **CLI control plane**: every running instance exposes a Unix control socket so the TUI, a dashboard, or an external controller can drive the swarm programmatically.

## Quick start

```bash
# 1. Run automated setup (installs Rust if needed, builds the orchestrator and container image)
./setup.sh

# 2. Start the TUI in swarm-demo mode (no model cost)
cd orchestrator
cargo run -- --swarm-demo

# 3. Or install the binary to PATH
cargo install --path .
eos --help
```

Inside the TUI:

- Type `/` to open the slash-command picker and filter commands.
- Type `@` to mention a model (e.g. `@kimi`, `@claude`).
- Press `Tab` to cycle layouts; `Ctrl+h` / `Ctrl+l` to move focus between panes.
- Press `PageUp` / `PageDown` to scroll the focused pane.

## Build and test

### Rust orchestrator

```bash
cd orchestrator
cargo build --release
cargo test                         # unit + integration tests
cargo test container_runner        # container-machine lifecycle tests (skip if container CLI missing)
```

### Swift sidecar

```bash
cd localmodel-lm
swift build                        # stub/coreai type-checking
./scripts/build-mlx.sh             # xcodebuild build for real MLX inference
./scripts/build-coreai.sh          # xcodebuild build for Core AI (requires Xcode-beta / macOS 27 SDK)
```

> Real MLX inference requires the xcodebuild path, not plain `swift build`, because MLX's Metal shaders must be compiled into a bundle next to the binary.

### Agent-runner container image

```bash
cd orchestrator
./deploy/run-eocon-agent-runner.sh build
./deploy/run-eocon-agent-runner.sh test
```

Requires the Apple `container` CLI and a running `container system` service. See [docs/CONTAINER_SANDBOX.md](docs/CONTAINER_SANDBOX.md).

## Running the orchestrator

```bash
# TUI
eos

# Headless instance
eos serve --name my-swarm

# Container-machine sandboxing
eos --runner container

# With a specific model variant
eos --model claude:opus-4.8

# CLI control plane examples
eos ps
eos exec "/agent spawn scout --role researcher"
eos exec --json "/swarm status"
eos snapshot
```

## Directory structure

```
Eocon-Orchestrator-swarm/
├── AGENTS.md                  # Agent behavior and planning rules for this repo
├── Constitution.md            # Governance, safety architecture, and oversight norms
├── NEXT_STEPS.mdx             # Cold-start handoff and prioritized next steps
├── PLAN_FORMAT.md             # Plan-file format specification
├── TESTING.md                 # Testing notes
├── docs/
│   └── CONTAINER_SANDBOX.md   # Apple Container Machine sandbox runbook
├── orchestrator/              # Rust crate `eocon-swarm`; binary name `eos`
│   ├── src/
│   │   ├── swarm/             # engine, agent, memory, gist, verifier, queue, commands
│   │   ├── policy/            # PACT policy and RiskLabel
│   │   ├── observability/     # trace, metrics, watchdog
│   │   ├── agents/            # AgentRunner trait, PTY/container runners, providers
│   │   └── ui/                # ratatui panels
│   ├── tests/                 # integration tests
│   └── deploy/                # Containerfile and helper for agent-runner image
├── localmodel-lm/             # SwiftPM sidecar for local MLX/Core AI inference
│   ├── Sources/localmodel-lm/
│   └── scripts/               # build-mlx.sh, build-coreai.sh
├── plans/                     # Design specs and migration plans
│   ├── eocon_swarm_private_memory_e9b065ff.plan.mdx
│   └── port_custom_mcp_to_apple_containerization.plan.mdx
├── scripts/                   # Python helpers (extract_replay_sessions.py, convert_gemma3_coreai.py, ...)
├── .deps/                     # External/local dependencies (e.g. coreai-models)
└── research_artifacts/        # Traces and validation outputs
```

## Important docs and plans

- [Constitution.md](Constitution.md) — governance rules, risk tiers, PACT policy, prohibited behaviors, and failure modes.
- [NEXT_STEPS.mdx](NEXT_STEPS.mdx) — current branch state, verified environment facts, and the prioritized backlog.
- [plans/eocon_swarm_private_memory_e9b065ff.plan.mdx](plans/eocon_swarm_private_memory_e9b065ff.plan.mdx) — full design specification for the private-memory swarm.
- [docs/CONTAINER_SANDBOX.md](docs/CONTAINER_SANDBOX.md) — how to build, run, and tune the container-machine sandbox.

## Status

`main` is the verified baseline: the Rust orchestrator tests pass and the MLX 2.x backend has been proven end-to-end. Active work (MLX 3.x migration, Core AI decode-loop validation) lives on feature branches and is tracked in `NEXT_STEPS.mdx`.
