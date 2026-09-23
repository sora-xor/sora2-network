#!/usr/bin/env python3
"""Offline integrity and unsigned-call checks for the local 4.8.10 candidate."""

import hashlib
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load(name):
    return json.loads((ROOT / name).read_text())


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def compact(value):
    if value < 1 << 6:
        return bytes([value << 2])
    if value < 1 << 14:
        return ((value << 2) | 1).to_bytes(2, "little")
    if value < 1 << 30:
        return ((value << 2) | 2).to_bytes(4, "little")
    size = (value.bit_length() + 7) // 8
    return bytes([((size - 4) << 2) | 3]) + value.to_bytes(size, "little")


def call_index(metadata, pallet_name, call_name):
    pallet = next(p for p in metadata["pallets"] if p["name"] == pallet_name)
    types = {entry["id"]: entry["type"] for entry in metadata["types"]["types"]}
    variants = types[pallet["calls"]["ty"]]["def"]["variant"]["variants"]
    call = next(v for v in variants if v["name"] == call_name)
    return bytes([pallet["index"], call["index"]])


def verify():
    names = set()
    for line in (ROOT / "SHA256SUMS").read_text().splitlines():
        expected, name = line.split("  ", 1)
        relative = Path(name)
        require(not relative.is_absolute() and ".." not in relative.parts, "Unsafe manifest path")
        require(name not in names, "Duplicate manifest path: " + name)
        names.add(name)
        require(sha256((ROOT / name).read_bytes()) == expected, "Hash mismatch: " + name)
    actual = {str(p.relative_to(ROOT)) for p in ROOT.rglob("*") if p.is_file()
              and not any(part in {"node_modules", "__pycache__"} for part in p.relative_to(ROOT).parts)}
    actual -= {"SHA256SUMS", "validation/package-check.json"}
    require(names == actual, "Unlisted or missing package files: " + str(names ^ actual))

    info = load("runtime-upgrade-4.8.10-info.json")
    candidate = info["candidate"]
    wasm = (ROOT / candidate["file"]).read_bytes()
    digest = sha256(wasm)
    require(candidate["sha256"] == digest and candidate["bytes"] == len(wasm), "Candidate mismatch")
    require(candidate["specVersion"] == 132 and candidate["transactionVersion"] == 131, "Unexpected version")
    require(info["submittedTransactions"] == 0 and info["privateKeysRead"] == 0, "Unexpected live action")

    rehearsal = load("validation/babe-repair-wasm-rehearsal.json")
    compatibility = load("validation/wasm-compatibility.json")
    require(rehearsal["status"] == compatibility["status"] == "passed", "Validation did not pass")
    require(rehearsal["candidate"]["sha256"] == digest, "Rehearsal used another Wasm")
    require(compatibility["inputs"]["candidate"]["sha256"] == digest, "Compatibility used another Wasm")
    require(compatibility["inputs"]["baseline"]["sha256"] == info["preflight"]["deployedSha256"], "Baseline mismatch")
    provenance = load("validation/source-provenance.json")
    require(sha256((ROOT / provenance["patch"]).read_bytes()) == provenance["patchSha256"], "Source patch mismatch")
    require(provenance["patchAppliesToCleanBase"] is True, "Clean-base source patch was not verified")
    if "--check-workspace-source" in sys.argv[1:]:
        for name, expected in provenance["files"].items():
            require(sha256((ROOT.parent / name).read_bytes()) == expected, "Workspace source changed: " + name)

    metadata = load("validation/baseline-metadata.json")["V14"]
    encoded = call_index(metadata, "System", "set_code") + compact(len(wasm)) + wasm
    require((ROOT / "set-code-call.scale").read_bytes() == encoded, "setCode SCALE payload mismatch")
    require(bytes.fromhex((ROOT / "set-code-call.hex").read_text().strip()[2:]) == encoded, "setCode hex mismatch")
    require(info["proposal"]["bytes"] == len(encoded), "Proposal length mismatch")
    proposal_hash = "0x" + hashlib.blake2b(encoded, digest_size=32).hexdigest()
    require(info["proposal"]["hash"] == proposal_hash, "Proposal hash mismatch")
    preimage = call_index(metadata, "Preimage", "note_preimage") + compact(len(encoded)) + encoded
    require(bytes.fromhex((ROOT / "preimage-note-call.hex").read_text().strip()[2:]) == preimage, "Preimage payload mismatch")

    report = {"status": "passed", "verifiedFiles": len(names), "candidateSha256": digest,
              "proposalHash": proposal_hash, "unsignedCallsMatchCandidate": True,
              "exactWasmRehearsalAndCompatibilityPassed": True,
              "workspaceSourceChecked": "--check-workspace-source" in sys.argv[1:], "networkRequests": 0}
    (ROOT / "validation/package-check.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    verify()
