#!/usr/bin/env python3
"""Extract the ordered maintenance work for live block authors from a rehearsal.

The bundle is the exact rehearsed envelope sequence after the first child's Tick:
work, audit, final-code, terminal-page and terminal envelopes, in replay order.
Every file is SHA-256 pinned. The runtime authenticates each envelope against the
release policy, so the bundle carries no authority of its own; `recovery_feeder.py`
delivers it to a validator's `--recovery-inherent-dir`.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import hashi_codec as scale
import release_package as release

TICK = b"snxrec01\x01\x00"


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def prepare(rehearsal: Path, destination: Path) -> dict:
    manifest_raw = (rehearsal / "manifest.json").read_bytes()
    manifest = json.loads(manifest_raw)
    scale.require(manifest.get("schema") == "sora-nexus-maintenance-replay-v1"
                  and manifest.get("checkpointHash") == scale.CHECKPOINT_HASH, "not a recovery rehearsal manifest")

    def payload(row: dict) -> bytes:
        raw = (rehearsal / row["file"]).read_bytes()
        scale.require(sha(raw) == row["sha256"], f"rehearsal file changed: {row['file']}")
        return raw

    policy = payload(manifest["policy"])
    steps = manifest["steps"]
    scale.require(steps[0].get("recovery") and payload(steps[0]["recovery"]) == TICK,
                  "the first child must carry only a Tick")
    envelopes, finished = [], False
    for step in steps[1:]:
        if step.get("recovery") is None:
            # Ordinary blocks follow only the terminal envelope.
            scale.require(step["expectedPhase"] == "Complete", "ordinary block before completion")
            finished = True
            continue
        scale.require(not finished, "work after completion")
        raw = payload(step["recovery"])
        scale.require(raw.startswith(b"snxrec01") and raw != TICK, "rehearsed step is not a work envelope")
        envelopes.append(raw)
    scale.require(finished and steps[-1]["expectedPhase"] == "Complete", "rehearsal does not reach completion")
    destination.mkdir(parents=False)
    rows = []
    for index, raw in enumerate(envelopes):
        name = f"{index:05}.scale"
        with (destination / name).open("xb") as stream:
            stream.write(raw)
        rows.append({"file": name, "bytes": len(raw), "sha256": sha(raw)})
    bundle = {"schema": "sora-nexus-recovery-work-bundle-v1", "checkpointHash": scale.CHECKPOINT_HASH,
              "checkpointNumber": 28_191_817,
              "policyCommitment": "0x" + release.policy_identity(policy)["commitment"],
              "rehearsalManifestSha256": sha(manifest_raw), "envelopes": rows}
    raw = (json.dumps(bundle, indent=1) + "\n").encode()
    with (destination / "BUNDLE.json").open("xb") as stream:
        stream.write(raw)
    return {"bundle": str(destination / "BUNDLE.json"), "bundleSha256": sha(raw), "envelopes": len(rows),
            "bytes": sum(row["bytes"] for row in rows), "policyCommitment": bundle["policyCommitment"]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rehearsal", type=Path, required=True, help="prepare_release_rehearsal.py destination")
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(prepare(args.rehearsal, args.destination), indent=2))
