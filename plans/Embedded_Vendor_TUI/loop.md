# Embedded Vendor TUI — Implementation Loops

Source plan: [`plan.md`](plan.md)
Source tech plan: [`tech-plan.md`](tech-plan.md)
Loop engineering reference: `file:Eocon/Research/loop-engineering`

This file defines the loops that implement the Embedded Vendor TUI in Eocon Orchestrator Swarm (`eos`) in three phases. Each loop is an implementer prompt followed by a verifier prompt. Run one phase at a time. Do not start Phase B until Phase A tests pass; do not start Phase C until Phase B's live attach demo passes.

---

## Active Loops

| Phase | Pattern | Status | Trigger | File/artifact produced |
|---|---|---|---|---|
| A — Terminal fidelity | Implementer + verifier | Not started | Manual | `term/` module, raw-chunk runner, resize propagation, emulator tests |
| B — Interactive embedding | Implementer + verifier | Blocked on A | Phase A verifier green | Terminals layout, passthrough mode, vendor adapters, worktrees |
| C — Structured event sidecar | Implementer + verifier | Blocked on B | Phase B verifier green + live attach demo | Tailer, hooks installer, AgentEvent ingestion, gist-from-event |

---

## Prerequisites

- `file:Dev/Tools/Eocon-Orchestrator-swarm/orchestrator/` builds: `cargo build` and `cargo test` green at baseline.
- At least one vendor CLI installed for live checks: `claude` (required for Phase B/C demos), optionally `codex`, `gemini`, `kimi`.
- For Phase C: a scratch git project directory the verifier can point Claude Code at without touching real work.
- The repo is its own git root (`file:Dev/Tools/Eocon-Orchestrator-swarm/`); never `git init` above it; commits scoped to `orchestrator/` and `plans/`.

---

## Phase A — Terminal Fidelity

### Implementer prompt

```text
Implement Phase A of the Embedded Vendor TUI per file:Dev/Tools/Eocon-Orchestrator-swarm/plans/Embedded_Vendor_TUI/plan.md Section 4, steps 1-9.

Changes:
1. Cargo.toml — add the vt100 crate (latest published version that builds in this workspace).
2. src/term/mod.rs + src/term/emulator.rs — TermEmulator wrapping vt100::Parser with the exact method surface PtyScreen exposes today (advance, resize, dims, as_text, full_text, scrollback offset) plus title(), cursor(), is_alt_screen().
3. src/term/widget.rs — TerminalPaneWidget painting the styled cell grid into the ratatui Buffer, with cursor rendering and scrollback windowing.
4. src/agents/runner.rs — replace the read_until(b'\n') reader loop (lines 101-119) with raw 8 KB chunk reads; add TERM=xterm-256color and COLORTERM=truecolor to the spawn env; add resize(handle, PtySize) to the AgentRunner trait implemented via handle.master.resize.
5. src/agents/sandbox.rs — apply the same reader and env changes to the container runner.
6. src/swarm/agent.rs — PtyOutput.screen becomes TermEmulator; delete src/pty_screen.rs; update all references (app.rs advance/parse sites, workflow_panel.rs); drop the vte dependency when nothing uses it.
7. src/app.rs — replace resize_screens with dims bookkeeping: the render pass records each PTY agent's inner pane dims; on tick, reconcile emulator.resize + runner.resize for any agent whose dims changed.
8. src/ui/workflow_panel.rs — render PTY agents through TerminalPaneWidget instead of flattening full_text().
9. tests/term_emulator.rs — alt-screen enter/leave restores primary grid; SGR truecolor+bold styling; absolute cursor addressing; resize without panic; a checked-in captured byte sample from a real TUI startup parses cleanly.

Constraints:
- Do not touch src/chat.rs (headless path is frozen).
- Do not change any input handling or key mapping in this phase.
- Keep the Swift build/test screen-parse path (app.rs ~723-733) working via the full_text() shim.
- cargo test, cargo clippy, cargo build must all pass.

After implementing, run the full test suite and manually spawn a build agent (existing flow) to confirm styled output renders. Report test results. Do not proceed to Phase B.
```

### Verifier prompt

```text
Verify Phase A of the Embedded Vendor TUI.

Checks:
1. Every file listed in plan.md Section 4 steps 1-9 exists (or is deleted where specified: src/pty_screen.rs gone).
2. cargo build && cargo test && cargo clippy -- -D warnings all pass.
3. grep confirms no read_until remains in src/agents/runner.rs or src/agents/sandbox.rs; the reader uses raw read() chunks.
4. The spawn env includes TERM=xterm-256color and COLORTERM=truecolor in both runners.
5. AgentRunner trait has resize(); app.rs calls it when pane dims change (no rows-4/cols÷2 heuristic remains).
6. tests/term_emulator.rs covers alt-screen, truecolor SGR, cursor addressing, and resize; all pass.
7. Live smoke: run eos, spawn an existing PTY build/test agent, confirm styled (colored) output renders in the workflow pane and PageUp scrollback still works.
8. src/chat.rs is untouched (git diff shows no changes).

Record results in STATE.md. If any check fails, open a worktree fix attempt or escalate to the operator. Max 3 fix attempts before escalation.
```

---

## Phase B — Interactive Embedding

### Implementer prompt

```text
Implement Phase B of the Embedded Vendor TUI per file:Dev/Tools/Eocon-Orchestrator-swarm/plans/Embedded_Vendor_TUI/plan.md Section 4, steps 10-18.

Changes:
10. src/term/keys.rs — table-driven encode_key(KeyEvent) -> Option<Vec<u8>> per the encoding table in tech-plan.md (printables, Ctrl-chords, Enter/Tab/Backspace/Esc, arrows with modifier variants, Home/End/PgUp/PgDn/Del/Ins, F1-F12).
11. src/app.rs — InputMode { Chrome, Passthrough { agent_id, pending_prefix } }; attach via Enter on a focused terminal pane or /agent attach <name>; Ctrl+q detaches; double Ctrl+q forwards a literal; child exit or kill while attached force-detaches.
12. src/event.rs + src/action.rs — passthrough branch before the Action map producing Action::ForwardKey(KeyEvent); Esc forwards in passthrough (quit is Chrome-only); App::update encodes and writes to the focused agent's RunHandle.writer with error surfacing, never blocking.
13. src/ui/layout.rs — SwarmLayout::Terminals in the Tab cycle: 30% chat rail + terminal grid (1 full / 2 vertical / 3-4 as 2x2); FocusedPane::Terminal(idx); Ctrl+h/l cycles rail and slots.
14. src/ui/terminal_pane.rs — pane chrome: title "<name> · <vendor> · <state>", state-colored border, accent border when attached, attach/detach hint.
15. src/agents/vendor.rs — VendorCli {Claude, Codex, Gemini, Kimi, Cursor} → AgentCmd: interactive argv (no -p/exec), exe discovery reusing the fallback paths in src/chat.rs:415-505, env TERM/COLORTERM/EOS_AGENT_ID/EOS_INSTANCE.
16. src/swarm/commands.rs + src/swarm/engine.rs — /agent spawn <name> --tui <vendor> [--cwd <path>] [--worktree] [-- <extra>]; spawn_tui_agent spawns at the target pane's dims and auto-switches to the Terminals layout; --worktree creates <project>/.eocon/worktrees/<name> via git worktree add (verbatim stderr on failure); /agent attach, /agent detach, /agent redraw (sends 0x0C); /agent kill --rm-worktree prunes.
17. src/ui/status_bar.rs + SLASH_COMMANDS in src/app.rs — PASSTHROUGH badge, hints, picker entries for the new subcommands.
18. tests/term_keys.rs + tests/passthrough_routing.rs — encoding table assertions; attach → forward → prefix detach → double-prefix literal → child-exit force-detach, using a capture writer.

Constraints:
- Esc must never quit the app while in Passthrough.
- Detach (Ctrl+q) must work even if the child is hung; detach never requires a PTY write.
- Do not touch src/chat.rs.
- cargo test, cargo clippy, cargo build green.

After implementing, run the live demo: eos → /agent spawn demo --tui claude → attach → run a trivial task inside Claude Code (approve its permission prompt natively) → detach → /agent kill demo. Report what worked and any rendering/input defects. Do not proceed to Phase C.
```

### Verifier prompt

```text
Verify Phase B of the Embedded Vendor TUI.

Checks:
1. Every file in plan.md Section 4 steps 10-18 exists; cargo build/test/clippy green.
2. tests/term_keys.rs and tests/passthrough_routing.rs pass, including double-prefix literal and child-exit force-detach cases.
3. Live attach demo: spawn --tui claude, attach, type a prompt, watch Claude Code's own UI (spinner, tool output, permission prompt) render inside the pane; approve the prompt natively; detach with Ctrl+q; the outer TUI chrome is intact throughout.
4. Esc pressed while attached goes to Claude Code (e.g. cancels its input), and does NOT quit eos.
5. Resize the host terminal and cycle layouts while the pane is live: the embedded TUI reflows itself (no truncated stale frame), proving master.resize propagation.
6. --worktree spawns in <project>/.eocon/worktrees/<name>; git worktree list shows it; kill --rm-worktree removes it.
7. Spawning a second pane switches the grid to two panes and focus cycling reaches both.
8. src/chat.rs untouched; @mention one-shot agents still stream into the chat panel as before.

Record results in STATE.md. Escalate after 3 fix attempts.
```

---

## Phase C — Structured Event Sidecar

### Implementer prompt

```text
Implement Phase C of the Embedded Vendor TUI per file:Dev/Tools/Eocon-Orchestrator-swarm/plans/Embedded_Vendor_TUI/plan.md Section 4, steps 19-24.

Changes:
19. src/observability/agent_events.rs — AgentEvent {agent_id, session_id, kind, payload, at}; AgentEventKind {SessionStart, Message, ToolUse, ToolResult, PermissionRequest, Stop}; bounded mpsc queue owned by SwarmEngine, drained on tick.
20. src/observability/transcript_tail.rs — per-agent tailer thread: cwd → slug (/ → -), poll ~/.claude/projects/<slug>/ every 500 ms, adopt the newest unclaimed *.jsonl with ctime ≥ agent spawn time (30 s adoption timeout → Voice::Warning), seek-tail appended lines, tolerant parse into AgentEvents, skip malformed lines with a counter, stop on agent kill.
21. src/control/protocol.rs + src/control/server.rs — ControlRequest::AgentEvent {agent, kind, payload} routed onto the engine queue.
22. src/hooks.rs + eos subcommands — eos hooks install [--project <path>]: merge-only write of the hooks object into .claude/settings.json (print diff first; refuse on parse failure); eos event --instance <n> --agent <id> --kind <k>: thin control-socket client reading hook JSON from stdin into payload. The Phase B vendor adapter already injects EOS_AGENT_ID/EOS_INSTANCE.
23. src/swarm/engine.rs — drain events per tick: Message → Thinking, ToolUse → Acting, Stop → Completed + GistDraft from the last assistant Message text → existing verifier → publish. Vendor TUI agents must never have gists built from screen text.
24. tests/transcript_tail.rs — checked-in fixture JSONL → exact AgentEvent sequence; malformed-line skip; ControlRequest::AgentEvent serde round-trip.

Constraints:
- No tokio; threads + mpsc + poll, matching the codebase.
- Hook failure must never block or break the vendor CLI (eos event exits fast and silent on socket absence).
- The tailer is advisory: pane interaction must work identically if it never adopts a transcript.
- Do not touch src/chat.rs.

After implementing, run the live demo: eos hooks install on a scratch project → spawn --tui claude --cwd <scratch> → complete a small task → confirm the agent card transitions Thinking/Acting/Completed and a verified gist publishes without any screen scraping. Report results.
```

### Verifier prompt

```text
Verify Phase C of the Embedded Vendor TUI.

Checks:
1. Every file in plan.md Section 4 steps 19-24 exists; cargo build/test/clippy green; tests/transcript_tail.rs passes.
2. Live: with hooks installed on a scratch project, a Claude Code pane session produces AgentEvents from BOTH channels (kill the tailer thread via a debug flag or temporarily rename the projects dir to prove hooks alone still deliver Stop).
3. The agent card lifecycle tracks the session (Thinking → Acting during tool use → Completed on stop) without consulting screen text — grep confirms no as_text()/full_text() call feeds gist extraction for vendor TUI agents.
4. A verified gist publishes on Stop with the final assistant text as its claim source.
5. eos hooks install on a settings.json that already has user content preserves that content byte-for-byte outside the hooks key, and refuses cleanly on malformed JSON.
6. A hook firing with no live instance socket exits non-zero fast and Claude Code proceeds unaffected.
7. Malformed JSONL lines in a doctored transcript are skipped (counter increments), never fatal.
8. Headless @mention agents still publish gists via the old path; src/chat.rs untouched.

Record results in STATE.md. Mark Phase C complete only after the live no-scrape gist demo succeeds end to end.
```

---

## Human Gates

- No Phase B work until the Phase A verifier reports all checks passed.
- No Phase C work until the Phase B live attach demo passes (check 3).
- No auto-merge to any protected branch; no commits outside `orchestrator/` and `plans/` in this repo.
- Screen scraping for vendor TUI agents is banned from Phase A onward — any implementer shortcut that parses pane text for orchestration state is a verifier-fail, not a deviation note.
- `eos hooks install` must never clobber user `.claude/settings.json` content; merge-only, diff shown.
- The headless path (`src/chat.rs`) is frozen for all three phases; any diff there is an automatic fail.

---

## State Tracking

Track in `file:Dev/Tools/Eocon-Orchestrator-swarm/plans/Embedded_Vendor_TUI/STATE.md`:

```markdown
## Embedded Vendor TUI Implementation State

### Phase A — Terminal fidelity
- Last run: YYYY-MM-DD HH:MM UTC
- Status: in_progress | passed | blocked
- Test status: ...

### Phase B — Interactive embedding
- Last run: ...
- Status: ...

### Phase C — Event sidecar
- Last run: ...
- Status: ...

### Blockers
- ...
```

---

## Failure Modes & Circuit Breakers

| Failure | Response |
|---|---|
| Same test fails 3 times across loop runs | Escalate to operator; do not auto-retry |
| vt100 crate does not build against the workspace | Stop; record versions tried; escalate with the alternative (tui-term pin or wezterm-term) rather than hand-extending pty_screen.rs |
| Live attach demo shows garbled rendering | File the failing byte capture as a tests/fixtures case first, then fix; never patch around it blind |
| Passthrough traps the operator (detach fails) | Immediate stop-ship for Phase B; detach is the safety invariant |
| Claude transcript schema drift breaks the fixture parser | Loosen the parser (skip unknown), keep hooks as primary; do not chase the schema in a loop |
| Vendor CLI updates change startup behavior mid-loop | Pin the vendor CLI version for the loop run; note it in STATE.md |
| Token budget hits 80% of daily cap | Switch to report-only for remaining runs |

---

## Budget

- Phase A: ~2-3 implementer runs; emulator fixtures dominate.
- Phase B: ~2-4 implementer runs; live-demo iteration dominates wall time.
- Phase C: ~2-3 implementer runs; hook/tailer integration is fiddly but small.
- Suggested daily token cap: 500K per phase.

---

## Links

- Plan: [`plan.md`](plan.md)
- Tech plan: [`tech-plan.md`](tech-plan.md)
- Host project: `file:Dev/Tools/Eocon-Orchestrator-swarm/orchestrator/`
- Format exemplar: `file:Eocon/Partners/AEI_Speakers_Inc/Planning/Airlock_Pipeline/`
- Loop engineering reference: `file:Eocon/Research/loop-engineering/LOOP.md`
