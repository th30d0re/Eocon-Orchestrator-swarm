# Shell Agent Library Handoff — Foundation Phase 5 Skills

**Date:** 2026-07-07  
**Model:** Kimi Code CLI  
**Repository:** `Eocon-Orchestrator-swarm/orchestrator`

## What Was Built

A reusable Rust package that wraps the Foundation Phase 5 shell agents
(`BuildAgent.sh`, `testAgent.sh`) so the Eocon Swarm can consume them as agent
skills. The package lives outside `src/` at the requested location and is wired
into the crate via Rust `#[path]` attributes.

## Files Added

| File | Purpose |
|------|---------|
| `agents_skills/mod.rs` | Top-level skill registry. |
| `agents_skills/foundation/mod.rs` | Shared `FoundationAgentOutput` envelope and module declarations. |
| `agents_skills/foundation/config.rs` | `FoundationConfig`: resolves `EOCON_FOUNDATION_ROOT` from env → on-disk config → default `~/Dev/Tools/Eocon-Foundation-V1`. |
| `agents_skills/foundation/BuildAgentSkill.rs` | Translates a generic `AgentCmd` into a concrete `BuildAgent.sh` invocation. |
| `agents_skills/foundation/TestAgentSkill.rs` | Translates a generic `AgentCmd` into a concrete `testAgent.sh` invocation. |
| `SHELL_AGENT_LIBRARY_HANDOFF.md` | This document. |

## Files Modified

| File | Change |
|------|--------|
| `src/lib.rs` | Declared `agents_skills` module via `#[path]`; updated CLI role list. |
| `src/config.rs` | Added `eocon_foundation_root: Option<String>` to `AppConfig`. |
| `src/swarm/agent.rs` | Added `AgentRole::FoundationBuilder` and `AgentRole::FoundationTester`. |
| `src/swarm/commands.rs` | Added role parsing for `foundation-builder` and `foundation-tester`. |
| `src/memory/store.rs` | Added system prompts for the new roles. |
| `src/app.rs` | Extended PTY output parsing to include Foundation roles. |
| `src/agents/swift.rs` | Added `foundation_build_agent_cmd` and `foundation_test_agent_cmd` helpers that use the skills. |

## Design Decisions

- **Thin wrappers, no source copy:** The skills resolve the existing Foundation
  scripts via `EOCON_FOUNDATION_ROOT` and invoke them with `--json`. The scripts
  themselves remain in `Eocon-Foundation-V1`.
- **Env → config → default resolution:** `FoundationConfig::load()` checks the
  `EOCON_FOUNDATION_ROOT` environment variable first, then the `eocon_foundation_root`
  field in `AppConfig`, then falls back to `~/Dev/Tools/Eocon-Foundation-V1`.
- **AgentCmd translation:** A caller builds a generic `AgentCmd` with program
  `BuildAgent` or `TestAgent`; the skill rewrites the program to the resolved
  script path and injects `EOCON_FOUNDATION_ROOT` into the environment.
- **Structured output reuse:** The Foundation scripts already emit the same JSON
  envelope that `agents/swift.rs` parses, so the Swarm ingests the result without
  a new parser.

## How to Use

### Spawn a Foundation agent from the TUI/CLI

```text
/agent spawn builder --role foundation-builder --provider anthropic --model claude-sonnet-4-6
/agent spawn tester --role foundation-tester --provider anthropic --model claude-sonnet-4-6
```

### Build a Foundation command programmatically

```rust
use eocon_swarm::agents::swift::{
    foundation_build_agent_cmd, foundation_test_agent_cmd,
};

let cmd = foundation_build_agent_cmd(
    "add password reset flow",
    Some("MyApp.xcodeproj"),
    Some("MyApp"),
    None,
    Some("anthropic"),
    Some("claude-sonnet-4-6"),
)?;
```

### Override the Foundation root

```bash
export EOCON_FOUNDATION_ROOT=/path/to/Eocon-Foundation-V1
eos agent spawn builder --role foundation-builder
```

Or set it in the on-disk config via `confy` (`eocon-swarm` app config).

## Verification

- `cargo check` passes.
- `cargo test` passes with 84 unit tests + integration tests.
- New unit tests cover:
  - Default `~` expansion for `EOCON_FOUNDATION_ROOT`.
  - Script path resolution.
  - `AgentCmd` program rewriting and argument/env preservation.
  - Failure when the resolved script is missing.
  - JSON envelope parsing.

## Known Limitations / Follow-up

- The skills validate that the resolved script exists at translation time. On a
  fresh machine without the Foundation checkout, agents will fail fast with a
  clear error.
- The existing `SwiftBuilder`/`SwiftTester` roles remain unchanged and still
  expect `BuildAgent`/`TestAgent` to be in `PATH`. The new Foundation roles are
  the explicit path for using the Foundation Phase 5 scripts.
- No shell wrapper scripts are provided yet; the package is currently Rust-only.
