#!/usr/bin/env python3
"""Check repository Markdown links and fenced code-block balance offline."""
import os
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
SKIP = {".git", "target", "__pycache__", "test_snapshots", "node_modules", ".venv"}
errors = []
checked = 0
for directory, folders, files in os.walk(ROOT):
    folders[:] = [name for name in folders if name not in SKIP]
    for name in files:
        if not name.endswith(".md"):
            continue
        path = Path(directory) / name
        checked += 1
        fenced = False
        for number, line in enumerate(path.read_text().splitlines(), 1):
            if re.match(r"^\s*```", line):
                fenced = not fenced
                continue
            if fenced:
                continue
            for target in re.findall(r"\[[^\]]*\]\(([^)\n]+)\)", line):
                target = target.strip().strip("<>")
                parsed = urlsplit(target)
                if parsed.scheme or parsed.netloc or not parsed.path:
                    continue
                destination = (path.parent / unquote(parsed.path)).resolve()
                if not destination.is_relative_to(ROOT) or not destination.exists():
                    errors.append(f"{path.relative_to(ROOT)}:{number}: missing local link {target}")
        if fenced:
            errors.append(f"{path.relative_to(ROOT)}: unclosed fenced code block")
if errors:
    raise SystemExit("\n".join(errors))
print(f"Checked local links and code fences in {checked} Markdown files.")
