#!/usr/bin/env python3
"""Verify the reviewed source graph and generated crypto bundle stay in sync."""
import hashlib
import json
from pathlib import Path
source = Path(__file__).resolve().parents[1]
output = source.parent / "polkadot-wasm-crypto-wasm"
repository = source.parents[1]
provenance = json.loads((output / "build-provenance.json").read_text())
for category, root in [("sources", source), ("assets", output)]:
    for name, expected in provenance[category].items():
        file = (root / name).resolve()
        if repository not in file.parents:
            raise SystemExit(f"Provenance path is outside the repository: {name}")
        if hashlib.sha256(file.read_bytes()).hexdigest() != expected:
            raise SystemExit(f"Crypto {category} changed without rebuilding: {name}")
print(f"Crypto provenance verified: {len(provenance['sources'])} source inputs and {len(provenance['assets'])} assets")
