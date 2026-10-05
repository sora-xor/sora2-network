#!/usr/bin/env python3
"""Hash every distributable package file, excluding dependencies and transient reports."""

import hashlib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
excluded_files = {"SHA256SUMS", "validation/package-check.json", "validation/.public-read-cache.json.gz"}
rows = []
for file in sorted(ROOT.rglob("*")):
    relative = file.relative_to(ROOT)
    if not file.is_file() or str(relative) in excluded_files or \
            any(part in {"node_modules", "__pycache__"} for part in relative.parts):
        continue
    rows.append(hashlib.sha256(file.read_bytes()).hexdigest() + "  " + str(relative))
(ROOT / "SHA256SUMS").write_text("\n".join(rows) + "\n")
print(f"Recorded {len(rows)} package files")
