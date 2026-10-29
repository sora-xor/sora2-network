#!/usr/bin/env python3
"""Bind the exact production build to an offline activation replay manifest.

The replay executes the built 134/135/138 Wasm against the authenticated
checkpoint with the composed work, staging, terminal audit and two ordinary
blocks after activation. It uses the retained checkpoint GRANDPA authorities and
slots after the abandoned head, with a day-long gap before the first work block
so the maintenance-duration gate is exercised. No keys, RPC, node DB or
publication are involved; the replay report remains separate release evidence.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import shutil

import hashi_codec as scale
import release_package as release

LAST_ABANDONED_SLOT = 298_871_044
SLOT_MS = 6000
EPOCH_SLOTS = 600
# The activation gate requires a day of maintenance: 14,400 slots, 24 epochs.
MAINTENANCE_GAP_SLOTS = 14_400
TIMESTAMP_NOW_KEY = "0x" + (bytes.fromhex("f0c365c3cf59d671eb72da0e7a4113c4")
                            + bytes.fromhex("9f1f0515f462cdcf84e0f1d6045dfcbb")).hex()


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def checkpoint_timestamp(capture: Path) -> int:
    """Timestamp.Now at the checkpoint, read from the authenticated capture pages."""
    manifest = json.loads((capture / "capture-complete.json").read_bytes())
    for page in manifest["top"]["pages"]:
        if not page["keyCount"] or not page["firstKey"] <= TIMESTAMP_NOW_KEY <= page["lastKey"]:
            continue
        raw = (capture / page["file"]).read_bytes()
        scale.require(sha(raw) == page["sha256"], "capture page hash differs")
        for line in gzip.decompress(raw).decode().splitlines()[1:]:
            key, value = json.loads(line)
            if key == TIMESTAMP_NOW_KEY:
                return int.from_bytes(bytes.fromhex(value[2:]), "little")
    raise ValueError("checkpoint Timestamp.Now missing from capture")


def prepare(build: Path, destination: Path, first_slot: int) -> dict:
    scale.require(first_slot > LAST_ABANDONED_SLOT, "first recovery slot must follow the abandoned head")
    composition = json.loads((build / "compose" / "stdout.log").read_bytes())
    consensus = json.loads((build / "checkpoint-consensus" / "stdout.log").read_bytes())
    policy = (build / "policy.scale").read_bytes()
    identity = release.policy_identity(policy)
    scale.require("0x" + identity["commitment"] == composition["policyCommitment"], "policy differs from composition")
    scale.require(consensus["checkpointHash"] == scale.CHECKPOINT_HASH and consensus["previousGrandpaSetId"] == 7956,
                  "checkpoint consensus differs")
    capture = build / "inputs" / "checkpoint"
    offset = checkpoint_timestamp(capture) % SLOT_MS
    destination.mkdir(parents=False)
    shutil.copytree(capture, destination / "checkpoint")

    def artifact(name: str, data: bytes) -> dict:
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as stream:
            stream.write(data)
        return {"file": name, "sha256": sha(data)}

    wasm = {}
    for role, directory, version in (("bootstrap", "build-134", (134, 131)),
                                     ("hold", "build-135", (135, 132)),
                                     ("finalRuntime", "build-final138", (138, 134))):
        data = (build / directory / f"runtime-{version[0]}.compact.compressed.wasm").read_bytes()
        actual = release.wasm_identity(data)
        scale.require((actual["specVersion"], actual["transactionVersion"]) == version, "wrong build Wasm identity")
        wasm[role] = artifact(f"runtime-{version[0]}.wasm", data)
    final_bytes = (destination / wasm["finalRuntime"]["file"]).read_bytes()

    steps, slot = [], first_slot - 1
    malformed = artifact("work/malformed.scale", b"snxrec01\x01\xfe")

    def step(name, data, phase, expected_spec=134, rejected=(), gap=1):
        nonlocal slot
        slot += gap
        payload = artifact(f"work/{name}.scale", data) if data is not None else None
        steps.append({"slot": slot, "timestamp": slot * SLOT_MS + offset, "authorityIndex": 0,
                      "recovery": payload, "rejectedRecovery": list(rejected),
                      "expectedSpec": expected_spec, "expectedPhase": phase})
        return payload

    # The production first child carries only a Tick, then a day passes.
    step("tick", b"snxrec01\x01\x00", "Applying")
    previous = None
    envelopes = composition["applyEnvelopes"]
    for index, payload in enumerate(envelopes):
        previous = step(f"apply-{index:05}", scale.raw(payload),
                        "Auditing" if index + 1 == len(envelopes) else "Applying",
                        rejected=[malformed] + ([previous] if previous else []),
                        gap=MAINTENANCE_GAP_SLOTS if index == 0 else 1)
    for index, payload in enumerate(composition["auditEnvelopes"]):
        step(f"audit-{index:05}", scale.raw(payload), "Auditing")
    previous = None
    for index, offset_bytes in enumerate(range(0, len(final_bytes), 65536)):
        chunk = final_bytes[offset_bytes:offset_bytes + 65536]
        # RecoveryInherent v1, RecoveryAction::StageCode(3), Final(0).
        payload = b"snxrec01\x01\x03\x00" + scale.u32(index) + scale.vec(chunk)
        previous = step(f"final-code-{index:05}", payload, "Auditing", rejected=[previous] if previous else [])
    for index, payload in enumerate(composition["terminalPreludeEnvelopes"]):
        step(f"terminal-page-{index:05}", scale.raw(payload), "Auditing")
    step("terminal", scale.raw(composition["terminalEnvelope"]), "Complete", expected_spec=138)
    step("ordinary", None, "Complete", expected_spec=138, gap=EPOCH_SLOTS)
    step("ordinary-following", None, "Complete", expected_spec=138)

    manifest = {"schema": "sora-nexus-maintenance-replay-v1", "checkpointHash": scale.CHECKPOINT_HASH,
                "checkpointCapture": "checkpoint", "policy": artifact("policy.scale", policy), **wasm,
                "repairs": [], "previousGrandpaSet": consensus["previousGrandpaSetId"],
                "freshGrandpa": consensus["grandpaAuthorities"], "steps": steps}
    raw = (json.dumps(manifest, indent=2) + "\n").encode()
    (destination / "manifest.json").write_bytes(raw)
    return {"manifest": str(destination / "manifest.json"), "manifestSha256": sha(raw), "steps": len(steps),
            "applyEnvelopes": len(envelopes), "auditEnvelopes": len(composition["auditEnvelopes"]),
            "finalCodeChunks": (len(final_bytes) + 65535) // 65536,
            "terminalPreludeEnvelopes": len(composition["terminalPreludeEnvelopes"]),
            "firstSlot": first_slot, "lastSlot": slot, "policyCommitment": composition["policyCommitment"],
            "productionReady": False}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--first-slot", type=int, required=True)
    args = parser.parse_args()
    print(json.dumps(prepare(args.build, args.destination, args.first_slot), indent=2))
