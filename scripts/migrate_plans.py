#!/usr/bin/env python3
"""
Migrate legacy Cursor plan files from .plan.md to .plan.mdx.

Keeps YAML frontmatter intact, normalizes it for richer metadata, and writes
the body as MDX. Does not delete originals unless --replace is passed.

Usage:
    python3 migrate_plans.py .cursor/plans
    python3 migrate_plans.py .cursor/plans --replace
    python3 migrate_plans.py .cursor/plans --dry-run
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import sys
from datetime import datetime
from pathlib import Path
from typing import Any


def parse_frontmatter(text: str) -> tuple[dict[str, Any], str]:
    """Split a file into YAML frontmatter and body.

    Frontmatter is delimited by `---` on its own line at the very start of the
    file and a matching `---` on its own line. Subsequent `---` lines inside the
    body (horizontal rules) are ignored.
    """
    if not text.startswith("---"):
        return {}, text

    lines = text.splitlines(keepends=True)
    # First line is the opening ---; find the closing --- on its own line.
    close_idx = None
    for idx, line in enumerate(lines[1:], start=1):
        if line.strip() == "---":
            close_idx = idx
            break

    if close_idx is None:
        return {}, text

    yaml_text = "".join(lines[1:close_idx]).strip()
    body = "".join(lines[close_idx + 1 :]).lstrip("\n")

    try:
        import yaml

        frontmatter = yaml.safe_load(yaml_text) or {}
    except ImportError:
        # Minimal fallback: parse simple key/value pairs only.
        frontmatter = {}
        for line in yaml_text.splitlines():
            if ":" in line and not line.strip().startswith("-"):
                key, _, value = line.partition(":")
                frontmatter[key.strip()] = value.strip().strip('"').strip("'")

    return frontmatter, body


def infer_frontmatter(body: str, source: Path) -> dict[str, Any]:
    """Generate default frontmatter for a plan file that lacks YAML headers."""
    name = source.stem.replace(".plan", "").replace("_", " ").title()
    # Try to extract the first H1 as the name.
    match = re.search(r"^#\s+(.+)$", body, re.MULTILINE)
    if match:
        name = match.group(1).strip()

    # First paragraph as overview.
    overview = ""
    paragraphs = re.split(r"\n\s*\n", body.strip())
    for para in paragraphs:
        para = para.strip()
        if para and not para.startswith("#") and not para.startswith("```"):
            overview = re.sub(r"\s+", " ", para)[:300]
            break

    return {
        "name": name,
        "overview": overview,
        "kind": "plan",
        "formatVersion": 1,
        "status": "draft",
        "isProject": False,
    }


def dump_frontmatter(frontmatter: dict[str, Any]) -> str:
    """Serialize frontmatter to YAML."""
    try:
        import yaml

        # Use literal block scalars for multiline strings where helpful.
        class LiteralBlockScalar(str):
            pass

        def literal_block_representer(dumper, data):
            return dumper.represent_scalar("tag:yaml.org,2002:str", data, style="|")

        yaml.add_representer(LiteralBlockScalar, literal_block_representer)

        converted = {}
        for key, value in frontmatter.items():
            if isinstance(value, str) and "\n" in value:
                converted[key] = LiteralBlockScalar(value)
            else:
                converted[key] = value

        return yaml.dump(converted, sort_keys=False, allow_unicode=True, default_flow_style=False)
    except ImportError:
        return _minimal_yaml_dump(frontmatter)


def _minimal_yaml_dump(data: dict[str, Any], indent: int = 0) -> str:
    """Minimal YAML serializer for when PyYAML is unavailable."""
    lines = []
    for key, value in data.items():
        prefix = "  " * indent
        if isinstance(value, dict):
            lines.append(f"{prefix}{key}:")
            lines.append(_minimal_yaml_dump(value, indent + 1))
        elif isinstance(value, list):
            lines.append(f"{prefix}{key}:")
            for item in value:
                if isinstance(item, dict):
                    first = True
                    for k, v in item.items():
                        if first:
                            lines.append(f"{prefix}- {k}: {v}")
                            first = False
                        else:
                            lines.append(f"{prefix}  {k}: {v}")
                else:
                    lines.append(f"{prefix}- {item}")
        elif isinstance(value, str) and "\n" in value:
            lines.append(f"{prefix}{key}: |")
            for line in value.splitlines():
                lines.append(f"{prefix}  {line}")
        else:
            lines.append(f"{prefix}{key}: {value}")
    return "\n".join(lines)


def normalize_frontmatter(frontmatter: dict[str, Any], source_path: Path) -> dict[str, Any]:
    """Add richer plan metadata while preserving existing fields."""
    normalized: dict[str, Any] = {}

    # Preserve ordering: name and overview first.
    if "name" in frontmatter:
        normalized["name"] = frontmatter.pop("name")
    if "overview" in frontmatter:
        normalized["overview"] = frontmatter.pop("overview")

    # Add kind/format if missing.
    if "kind" not in frontmatter:
        normalized["kind"] = "plan"
    if "formatVersion" not in frontmatter:
        normalized["formatVersion"] = 1

    # Status defaults.
    if "status" not in frontmatter:
        normalized["status"] = "draft"

    # Preserve legacy isProject field.
    if "isProject" in frontmatter:
        normalized["isProject"] = frontmatter.pop("isProject")

    # Preserve todos, normalizing statuses.
    if "todos" in frontmatter:
        todos = frontmatter.pop("todos")
        normalized["todos"] = [_normalize_todo(t) for t in todos if isinstance(t, dict)]

    # Preserve optional fields.
    for key in ["priority", "owner", "tags", "createdAt", "updatedAt", "dueDate", "reviewers"]:
        if key in frontmatter:
            normalized[key] = frontmatter.pop(key)

    # Add migration timestamp if not present.
    if "updatedAt" not in normalized:
        normalized["updatedAt"] = datetime.now().isoformat(timespec="seconds")

    # Preserve any unknown fields at the end.
    for key, value in frontmatter.items():
        if key not in normalized:
            normalized[key] = value

    return normalized


def _normalize_todo(todo: dict[str, Any]) -> dict[str, Any]:
    """Ensure every todo has id/content/status."""
    normalized = dict(todo)
    if "id" not in normalized:
        normalized["id"] = _slugify(normalized.get("content", "task"))
    if "status" not in normalized:
        normalized["status"] = "pending"
    # Normalize status vocabulary.
    status_map = {
        "not_started": "pending",
        "in progress": "in_progress",
        "in-progress": "in_progress",
        "done": "done",
        "completed": "done",
        "blocked": "blocked",
        "skipped": "skipped",
        "n/a": "skipped",
    }
    normalized["status"] = status_map.get(str(normalized["status"]).lower(), normalized["status"])
    return normalized


def _slugify(text: str) -> str:
    """Convert text to a kebab-case id."""
    text = text.lower()
    text = re.sub(r"[^a-z0-9]+", "-", text)
    return text.strip("-")[:32] or "task"


def needs_mdx_wrapper(body: str) -> bool:
    """Check whether the body already contains MDX JSX blocks."""
    return bool(re.search(r"<\/?[A-Z][A-Za-z0-9]*", body))


def convert_plan(source: Path, replace: bool = False, dry_run: bool = False) -> Path | None:
    """Convert a single .plan.md file to .plan.mdx."""
    if source.suffixes != [".plan", ".md"]:
        print(f"Skipping non-plan file: {source}")
        return None

    target = source.with_suffix("").with_suffix(".plan.mdx")
    if target.exists() and not replace and not dry_run:
        print(f"Skipping (already exists): {target}")
        return None

    text = source.read_text(encoding="utf-8")
    frontmatter, body = parse_frontmatter(text)
    if not frontmatter:
        print(f"  No YAML frontmatter found; inferring defaults for: {source.name}")
        frontmatter = infer_frontmatter(body, source)
    normalized = normalize_frontmatter(frontmatter, source)

    # If the body is plain Markdown with no JSX, we keep it as-is.
    # MDX parsers treat standard Markdown as valid MDX.
    output_lines = ["---"]
    output_lines.append(dump_frontmatter(normalized).rstrip())
    output_lines.append("---")
    output_lines.append("")

    # Optional: add a small format notice comment at the top of the body.
    # Use MDX comment syntax ({/* ... */}) because HTML comments are invalid in MDX.
    # Do not include the literal ".plan.md" suffix in the comment text so that
    # reference-update passes do not rewrite it to ".plan.mdx".
    if not needs_mdx_wrapper(body):
        output_lines.append("{{/* Migrated source: {old_stem}. Body is valid Markdown/MDX. */}}".format(old_stem=source.stem))
        output_lines.append("")

    output_lines.append(body.rstrip())
    output_lines.append("")

    output = "\n".join(output_lines)

    if dry_run:
        print(f"[dry-run] Would write: {target}")
        return target

    target.write_text(output, encoding="utf-8")
    print(f"Wrote: {target}")

    if replace:
        backup = source.with_suffix(source.suffix + ".bak")
        shutil.move(str(source), str(backup))
        print(f"Moved original to: {backup}")

    return target


def update_markdown_references(root: Path, dry_run: bool = False) -> list[Path]:
    """Update .plan.md references to .plan.mdx in READMEs and summaries."""
    updated: list[Path] = []
    for path in root.iterdir():
        if not path.is_file():
            continue
        if path.suffix.lower() not in {".md", ".mdx"}:
            continue
        text = path.read_text(encoding="utf-8")
        # Replace .plan.md links, but do not touch .plan.mdx, .plan.md.bak, etc.
        new_text = re.sub(r"\.plan\.md(?![\w.])", ".plan.mdx", text)
        if new_text != text:
            if not dry_run:
                path.write_text(new_text, encoding="utf-8")
            print(f"{'[dry-run] Would update' if dry_run else 'Updated'} references in: {path}")
            updated.append(path)
    return updated


def main() -> int:
    parser = argparse.ArgumentParser(description="Migrate .plan.md files to .plan.mdx")
    parser.add_argument("directory", help="Directory containing .plan.md files")
    parser.add_argument("--replace", action="store_true", help="Move originals to .plan.md.bak")
    parser.add_argument("--dry-run", action="store_true", help="Show what would happen without writing files")
    args = parser.parse_args()

    directory = Path(args.directory).resolve()
    if not directory.is_dir():
        print(f"Error: not a directory: {directory}", file=sys.stderr)
        return 1

    plans = sorted(directory.glob("*.plan.md"))
    if not plans:
        print(f"No .plan.md files found in {directory}")
        return 0

    print(f"Found {len(plans)} plan file(s) in {directory}\n")

    for plan in plans:
        convert_plan(plan, replace=args.replace, dry_run=args.dry_run)

    print(f"\nUpdating references in {directory}...")
    update_markdown_references(directory, dry_run=args.dry_run)

    return 0


if __name__ == "__main__":
    sys.exit(main())
