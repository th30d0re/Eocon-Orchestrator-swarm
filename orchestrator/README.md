# eocon-swarm

A Rust TUI multi-agent swarm orchestrator with isolated private memory and verified shared gists.

## Concepts

- **Private memory**: each agent has its own `AgentMemory` (chat history + system prompt). Other agents cannot read it.
- **Gists**: compressed, evidence-backed claims that agents publish after verification.
- **GistBoard**: the only shared context between agents — verified gists only.
- **Workflow Panel**: live agent cards, status, selected-agent detail.
- **Chat Panel**: user commands and verified gists.
- **PACT policy**: hierarchical safety — immutable global policy + user policy + CoT classify→act.
- **Observability**: nested traces, CSR/MG metrics, stall detection.

## Build

```bash
cd Eocon-Orchestrator-swarm/orchestrator
cargo build --release
```

## Run

```bash
# Mock mode for UI validation (no model calls)
./target/release/eos --swarm-demo

# Normal mode (TUI)
./target/release/eos

# Headless instance — no TUI, controlled entirely via the CLI
./target/release/eos serve --name my-swarm
```

## CLI control plane

Every running instance (TUI or headless) exposes its full command surface
over a Unix control socket (`~/.eocon/run/eos-<pid>.sock`) and registers
itself in `~/.eocon/instances/`. Anything the TUI can do, the CLI can do —
so AI models can drive the swarm programmatically while you watch.

```bash
eos ps                                   # list running instances (--json)
eos dashboard                            # live TUI dashboard over all instances

# Raw commands — identical to typing into the TUI prompt
eos exec "/agent spawn worker --role coder"
eos exec "build the auth module"         # natural input enqueues a swarm task
eos exec --json "/swarm status"          # machine-readable replies

# Sugar subcommands (same dispatch path as the slash commands)
eos agent spawn scout --role researcher
eos agent list
eos swarm run "map the codebase"
eos status
eos gist list
eos model list
eos metrics
eos quit                                 # sends /quit — shuts the instance down

# Full JSON state (agents, gists, metrics, activity log)
eos snapshot
```

With multiple instances running, target one with `--instance <name|pid>`
(`-i`); with a single instance the target is implicit. Flags (`--json`,
`-i`) go before the command words. Exit code is non-zero when the command
produced an error reply.

Commands arriving over the socket are echoed into the instance's chat log
as `cli> <command>`, so the TUI and `eos dashboard` show exactly what an
external controller (e.g. a model) is doing.

## Trace replay provider (offline)

The `traces` provider replays real Claude Code sessions instead of calling a
model — deterministic, offline, free. Use it to exercise the full pipeline
(task → response → gist → verifier → board → metrics) in tests and demos.

```bash
# One-time: extract replayable sessions from the local
# Complete-FABLE.5-traces-2M dataset clone (writes ~/.eocon/traces/sessions.jsonl)
python3 ../scripts/extract_replay_sessions.py

eos model ping traces        # preflight the sessions file
eos agent spawn replaybot --provider traces --model ~/.eocon/traces/sessions.jsonl
eos swarm run "any task"     # same prompt always replays the same session
```

Replayed responses are prefixed `Replayed trace <id>: …` so they can't be
mistaken for live model output.

## Dashboard

`eos dashboard` shows all live instances (left) and the selected instance's
phase, agents, gists, metrics, and activity log (right), refreshing every
second. Keys: `up/down` select instance, `r` refresh, `q` quit.

## Slash commands

- `/agent spawn <name> [--role <role>] [--provider <p>] [--model <m>]`
- `/agent list|pause|resume|edit|restart|kill|unfold <name>`
- `/gist list|unfold|reject <id>`
- `/swarm run|stop|reset|status <args>`
- `/policy` — show current policy
- `/trace <agent_id>` — show trace placeholder
- `/metrics` — show swarm metrics
- `/layout` — toggle layout
- `/help` — command list
- `/quit` — exit

## Architecture

Forked from the Foundation Orchestrator (`Eocon-Foundation-V1/.Foundation/Agents/phases/phase5/orchestrator/`), then extended with:

- `swarm/` — agents, memory, gists, verifier, task queue, engine, commands
- `policy/` — global/user policy, PACT classify→act engine
- `observability/` — traces, metrics, watchdog
- `ui/` — chat panel, workflow panel, agent cards, gist widgets
- `agents/` — `AgentRunner` trait, PTY backend, Swift build/test wrappers, Core AI/MLX provider stub, v2 sandbox placeholder

## On-device inference

`localmodel-lm/` is a SwiftPM sidecar skeleton for Core AI and MLX backends. It exposes a JSON-lines stdin/stdout protocol identical to the provider CLI contract used by `chat.rs`.

## v2 roadmap

- Real Core AI / MLX inference in `localmodel-lm`
- Apple Container Machine sandboxing (`ContainerMachineRunner`)
- MLX QLoRA fine-tune → eval → rollback loop
- Guided generation for gists/verdicts via Foundation Models
