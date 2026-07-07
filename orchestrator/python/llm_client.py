#!/usr/bin/env python3
"""Subprocess/JSON-lines bridge to the unified Rust LLM client.

This wrapper spawns the `eos llm` CLI and speaks JSON over stdin/stdout.
It is the Phase 1 Python binding; a future PyO3 native module can replace
it without changing the public API.

Expected usage:

    from llm_client import LlmClient

    client = LlmClient()
    response = client.complete(
        provider="anthropic",
        model="claude-sonnet-4-6",
        messages=[{"role": "user", "content": "Hello"}],
    )
    print(response["content"])
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from typing import Any, Iterator


class LlmError(RuntimeError):
    """Raised when the unified LLM client fails."""


class LlmClient:
    """Thin Python wrapper around the Rust `eos llm` CLI."""

    def __init__(self, binary: str | None = None) -> None:
        self._binary = binary or self._find_binary()

    def _find_binary(self) -> str:
        """Locate the `eos` binary.

        Search order:
        1. Environment variable `EOCON_LLM_BINARY`.
        2. `eos` on PATH.
        3. Cargo target/debug binary relative to this file.
        """
        env = os.environ.get("EOCON_LLM_BINARY")
        if env:
            return env

        if path := shutil.which("eos"):
            return path

        # Fallback: cargo debug build next to this file.
        here = os.path.dirname(os.path.abspath(__file__))
        candidate = os.path.join(
            here, "..", "target", "debug", "eos"
        )
        if os.path.isfile(candidate):
            return candidate

        raise LlmError("Could not find `eos` binary; set EOCON_LLM_BINARY")

    def _run(self, command: str, request: dict[str, Any]) -> subprocess.Popen:
        """Spawn the Rust CLI with the given request on stdin."""
        return subprocess.Popen(
            [self._binary, "llm", command],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )

    def complete(self, **kwargs: Any) -> dict[str, Any]:
        """Send a complete request and return the JSON response."""
        proc = self._run("complete", kwargs)
        payload = json.dumps(kwargs, ensure_ascii=False) + "\n"
        stdout, stderr = proc.communicate(input=payload, timeout=300)

        if proc.returncode != 0:
            raise LlmError(f"eos llm complete failed: {stderr.strip()}")

        try:
            return json.loads(stdout)
        except json.JSONDecodeError as exc:
            raise LlmError(f"invalid JSON from eos llm complete: {stdout[:200]}") from exc

    def stream(self, **kwargs: Any) -> Iterator[dict[str, Any]]:
        """Send a streaming request and yield JSON-lines events."""
        proc = self._run("stream", kwargs)
        payload = json.dumps(kwargs, ensure_ascii=False) + "\n"
        assert proc.stdin is not None
        proc.stdin.write(payload)
        proc.stdin.close()
        assert proc.stdout is not None

        for line in proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError as exc:
                raise LlmError(f"invalid JSON line from eos llm stream: {line[:200]}") from exc

        proc.wait(timeout=5)
        if proc.returncode != 0:
            stderr = proc.stderr.read() if proc.stderr else ""
            raise LlmError(f"eos llm stream failed: {stderr.strip()}")


def main() -> int:
    """Minimal CLI demo for the Python wrapper."""
    import argparse

    parser = argparse.ArgumentParser(description="Python bridge to the unified LLM client")
    parser.add_argument("--provider", default="anthropic")
    parser.add_argument("--model", default="")
    parser.add_argument("--max-tokens", type=int, default=3500)
    parser.add_argument("prompt", nargs="?", default="Hello")
    args = parser.parse_args()

    client = LlmClient()
    try:
        response = client.complete(
            provider=args.provider,
            model=args.model,
            messages=[{"role": "user", "content": args.prompt}],
            max_tokens=args.max_tokens,
        )
        print(response.get("content", ""))
    except LlmError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
