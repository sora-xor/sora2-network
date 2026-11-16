#!/usr/bin/env python3
"""Reject stale executor assets when reviewed source, lockfiles, or binary output changes."""
import hashlib
import json
from pathlib import Path

package = Path(__file__).resolve().parents[1]
repository = package.parents[1]
provenance = json.loads((package / "build-provenance.json").read_text())
for category in ["sources", "assets"]:
    for name, expected in provenance[category].items():
        file = (package / name).resolve()
        if repository not in file.parents:
            raise SystemExit(f"Provenance path is outside the repository: {name}")
        actual = hashlib.sha256(file.read_bytes()).hexdigest()
        if actual != expected:
            raise SystemExit(f"Executor {category} changed without rebuilding: {name}")
print(f"Executor provenance verified: {len(provenance['sources'])} source inputs and {len(provenance['assets'])} assets")
