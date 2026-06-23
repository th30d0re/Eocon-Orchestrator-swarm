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
./target/release/eocon-swarm --swarm-demo

# Normal mode
./target/release/eocon-swarm
```

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
