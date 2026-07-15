# Embedded Vendor TUI Implementation State

## Phase A — Terminal Fidelity

- Last run: —
- Status: **not started**
- Verifier: —

### Checklist

| # | Check | Result | Notes |
|---|---|---|---|
| 1 | Files from plan.md steps 1-9 exist; pty_screen.rs deleted | ⬜ | |
| 2 | cargo build / test / clippy green | ⬜ | |
| 3 | Raw-chunk PTY reads (no read_until) in both runners | ⬜ | |
| 4 | TERM/COLORTERM in spawn env | ⬜ | |
| 5 | AgentRunner::resize wired to pane-dims reconciliation | ⬜ | |
| 6 | Emulator fixture tests pass (alt-screen, SGR, cursor, resize) | ⬜ | |
| 7 | Live smoke: styled build-agent output + scrollback | ⬜ | |
| 8 | src/chat.rs untouched | ⬜ | |

## Phase B — Interactive Embedding

- Last run: —
- Status: **blocked on Phase A**

### Checklist

| # | Check | Result | Notes |
|---|---|---|---|
| 1 | Files from plan.md steps 10-18 exist; build/test/clippy green | ⬜ | |
| 2 | Key-encoding + passthrough routing tests pass | ⬜ | |
| 3 | Live attach demo: Claude Code interactive inside a pane | ⬜ | |
| 4 | Esc forwards in passthrough; never quits eos | ⬜ | |
| 5 | Live resize/layout cycle reflows the embedded TUI | ⬜ | |
| 6 | --worktree create + kill --rm-worktree prune | ⬜ | |
| 7 | Two-pane grid + focus cycling | ⬜ | |
| 8 | Headless @mention path unchanged | ⬜ | |

## Phase C — Structured Event Sidecar

- Last run: —
- Status: **blocked on Phase B**

### Checklist

| # | Check | Result | Notes |
|---|---|---|---|
| 1 | Files from plan.md steps 19-24 exist; build/test/clippy green | ⬜ | |
| 2 | Events arrive from both channels (tailer AND hooks) | ⬜ | |
| 3 | Lifecycle from events only; no screen text feeds TUI-agent gists | ⬜ | |
| 4 | Verified gist publishes on Stop | ⬜ | |
| 5 | hooks install is merge-only and refuses malformed JSON | ⬜ | |
| 6 | Hook with no socket exits fast; vendor CLI unaffected | ⬜ | |
| 7 | Malformed JSONL skipped, never fatal | ⬜ | |
| 8 | Headless gist path unchanged | ⬜ | |

## Blockers

- None recorded. Phase A is ready to start.
