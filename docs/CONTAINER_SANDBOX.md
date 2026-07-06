# Apple Container Machine Sandbox

The `eocon-swarm` orchestrator can run agent commands inside Linux micro-VMs
managed by the Apple `container` CLI instead of executing them directly on the
host. This gives each agent a hardened boundary while keeping the existing PTY
runner available as the default.

## Quick start

1. Install the Apple `container` CLI and start its system service:

   ```bash
   container system start
   ```

2. Build the agent-runner image:

   ```bash
   cd Eocon-Orchestrator-swarm/orchestrator
   ./deploy/run-eocon-agent-runner.sh build
   ```

3. Run the orchestrator with the container backend:

   ```bash
   eos --runner container
   ```

## Selecting the backend

The execution backend is chosen with the `--runner` flag:

```bash
eos --runner pty      # default: local PTY execution
eos --runner container # Apple Container Machine sandboxing
```

Resolution order: CLI flag → `runner_backend` in config → `EOCON_RUNNER_BACKEND`
environment variable → `pty` default. If `--runner container` is requested but
the `container` CLI is unavailable, the orchestrator prints a warning and falls
back to PTY.

## Machine topology

Agents are grouped by PACT risk tier:

| Risk tier | Machine | Lifecycle |
|-----------|---------|-----------|
| `Safe`, `Low` | `eocon-pool` | Created once, reused across agents, survives `kill_agent` |
| `Medium`, `High`, `Critical` | `eocon-agent-<uuid>` | Created per agent, stopped and deleted on `kill_agent` |

The shared pool keeps overhead low for the common case (reading files, running
linters, publishing gists). Dedicated machines give higher-risk agents a
separate kernel, memory cap, and disk boundary.

## Image build

The helper script builds the `eocon-agent-runner:latest` image used by both
pooled and dedicated machines:

```bash
./deploy/run-eocon-agent-runner.sh build
```

The image is based on `python:3.11-slim` and includes Rust, `ffmpeg`, `git`,
`curl`, build tools, and `sysvinit-core` so the machine can boot with `/sbin/init`.

Other useful subcommands:

```bash
./deploy/run-eocon-agent-runner.sh test           # smoke-test inside an ephemeral container
./deploy/run-eocon-agent-runner.sh machine-create # create the persistent eocon-pool machine
./deploy/run-eocon-agent-runner.sh machine-run    # open a shell in eocon-pool
./deploy/run-eocon-agent-runner.sh machine-test   # smoke-test inside eocon-pool
```

## Resource tuning

`ContainerMachineRunner` uses `SandboxConfig` to set defaults:

| Field | Default | Maps to |
|-------|---------|---------|
| `cpus` | 4 | `container machine set --name <n> cpus=4` |
| `memory_gb` | 8 | `container machine set --name <n> memory=8G` |
| `home_mount_readonly` | true | `container machine set --name <n> home-mount=ro` |

These are applied when a machine is first created. To change them, delete the
existing machine and let the runner recreate it with the updated config.

## Project and memory mounts

The runner passes `-w <project_dir>` so guest paths match the host layout. The
project directory is mounted read-only via `home-mount=ro`. Each agent's private
memory directory (`.eocon/memory/<agent>/`) is intended to be mounted writable;
this path is configured via `SandboxConfig::writable_memory_dir` and is applied
as a volume mount to the `container machine run` invocation.

## Fallback to PTY

If the `container` CLI is missing, the image build fails, or the operator simply
wants local execution, PTY mode is unchanged:

```bash
eos --runner pty
```

The `AgentRunner` trait keeps both backends interchangeable, so higher-level
engine code does not need to know which backend is active.

## Verification

Integration tests live in `orchestrator/tests/container_runner_test.rs` and
verify:

- The agent-runner image builds.
- `Low` tier agents run on the shared `eocon-pool` machine.
- The pooled machine survives `kill_agent`.
- `Medium` tier agents get a dedicated `eocon-agent-*` machine.
- Dedicated machines are removed within 10 seconds of `kill_agent`.
- `PtyRunner` still produces output unchanged.

Run them with:

```bash
cd Eocon-Orchestrator-swarm/orchestrator
cargo test container_runner
```

If the `container` CLI is not installed, the container tests skip automatically.

## Limitations

- Apple Container Machines are Linux micro-VMs and do **not** expose the host GPU
  or Neural Engine. The `localmodel-lm` inference sidecar must stay on macOS and
  be reached from the guest over the network or a stdio bridge.
- The first boot of a machine can take several seconds; subsequent commands on a
  pooled machine reuse the already-booted VM.
- Machine creation and deletion are shell-outs to the `container` CLI; robustness
  depends on the CLI exit codes and output format.
