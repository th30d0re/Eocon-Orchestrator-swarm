#!/usr/bin/env python3
"""Extract replayable Claude Code sessions from the Complete-FABLE.5-traces-2M
dataset for the eocon-swarm `traces` provider.

Filters the two sources that contain genuine Claude Code session logs, groups
events by sessionId, and writes one replayable session per JSONL line:

    {"session_id": "...", "task": "<first user prompt>", "chunks": ["...", ...]}

The swarm's `traces` provider (chat.rs) streams a session's chunks back as the
agent response, deterministically selected by hashing the task prompt — so the
whole orchestration pipeline (task -> response -> gist -> verifier -> board)
runs offline with realistic content and zero model calls.

Usage:
    python3 scripts/extract_replay_sessions.py \
        [--parquet PATH] [--out PATH] [--max-chars N] [--min-chunks N]
"""

import argparse
import json
import sys
from pathlib import Path

try:
    import pyarrow.parquet as pq
except ImportError:
    sys.exit("pyarrow is required: pip3 install pyarrow")

TRACE_SOURCES = {
    "1EYE4ALL/Fable-5-traces",
    "armand0e/claude-fable-5-claude-code",
}

DEFAULT_PARQUET = "/Users/emmanuel/Dev/Tools/Complete-FABLE.5-traces-2M/data/train.parquet"
DEFAULT_OUT = str(Path.home() / ".eocon" / "traces" / "sessions.jsonl")


def render_assistant_blocks(message, max_tool_chars=160):
    """Flatten an assistant message's content blocks into displayable text
    pieces. Keeps text and tool-use summaries; drops thinking/signature and
    fallback bookkeeping blocks."""
    pieces = []
    content = message.get("content")
    if isinstance(content, str):
        if content.strip():
            pieces.append(content.strip())
        return pieces
    if not isinstance(content, list):
        return pieces
    for block in content:
        if not isinstance(block, dict):
            continue
        btype = block.get("type")
        if btype == "text":
            text = (block.get("text") or "").strip()
            if text:
                pieces.append(text)
        elif btype == "tool_use":
            name = block.get("name", "tool")
            try:
                arg = json.dumps(block.get("input", {}))[:max_tool_chars]
            except (TypeError, ValueError):
                arg = "?"
            pieces.append(f"[tool:{name}] {arg}")
    return pieces


def first_user_text(message):
    """The task prompt: plain-string user content only (tool_result rows have
    list content and are not prompts)."""
    content = message.get("content")
    if isinstance(content, str) and content.strip():
        return content.strip()
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--parquet", default=DEFAULT_PARQUET)
    parser.add_argument("--out", default=DEFAULT_OUT)
    parser.add_argument("--max-chars", type=int, default=6000,
                        help="cap on total replayed characters per session")
    parser.add_argument("--min-chunks", type=int, default=1,
                        help="drop sessions with fewer assistant pieces")
    args = parser.parse_args()

    pf = pq.ParquetFile(args.parquet)
    sessions = {}  # sid -> {"task": str|None, "events": [(ts, kind, payload)]}
    scanned = kept_rows = 0

    for batch in pf.iter_batches(
        batch_size=20000, columns=["first_source_dataset", "row_json"]
    ):
        for source, row_json in zip(
            batch.column(0).to_pylist(), batch.column(1).to_pylist()
        ):
            scanned += 1
            if source not in TRACE_SOURCES:
                continue
            try:
                row = json.loads(row_json)
            except json.JSONDecodeError:
                continue
            rtype = row.get("type")
            if rtype not in ("user", "assistant"):
                continue
            sid = row.get("sessionId")
            message = row.get("message")
            ts = row.get("timestamp") or ""
            if not sid or not isinstance(message, dict):
                continue
            kept_rows += 1
            entry = sessions.setdefault(sid, {"task": None, "events": []})
            entry["events"].append((ts, rtype, message))

    out_sessions = []
    for sid, entry in sessions.items():
        events = sorted(entry["events"], key=lambda e: e[0])
        task = None
        chunks = []
        for _, kind, message in events:
            if kind == "user" and task is None:
                task = first_user_text(message)
            elif kind == "assistant":
                chunks.extend(render_assistant_blocks(message))
        if not task or len(chunks) < args.min_chunks:
            continue
        # Header line doubles as the gist claim: short, factual, marked as a
        # replay so nobody mistakes it for a live model answer.
        header = f"Replayed trace {sid[:8]}: {task[:140].splitlines()[0]}"
        body, total = [], 0
        for piece in chunks:
            if total + len(piece) > args.max_chars:
                break
            body.append(piece)
            total += len(piece)
        if not body:
            continue
        out_sessions.append(
            {"session_id": sid, "task": task, "chunks": [header] + body}
        )

    out_sessions.sort(key=lambda s: s["session_id"])
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("w") as f:
        for s in out_sessions:
            f.write(json.dumps(s) + "\n")

    print(f"scanned {scanned:,} rows, kept {kept_rows:,} session events")
    print(f"wrote {len(out_sessions):,} replayable sessions to {out_path}")
    print(f"size: {out_path.stat().st_size / 1024 / 1024:.1f} MB")


if __name__ == "__main__":
    main()
