#!/usr/bin/env python3
"""Assemble the recovery chain-spec RecoveryConfig from a passed production build.

Every value comes from the build's own outputs: the emitted policy, the three
compressed runtimes, the derived checkpoint consensus and the public abandoned
slot capture. Nothing is supplied by hand. `framenode prepare-recovery-spec`
then binds this config and the exact bootstrap Wasm into the release spec.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import hashi_codec as scale
import release_package as release

B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def blake(data: bytes) -> str:
    return "0x" + hashlib.blake2b(data, digest_size=32).hexdigest()


def ss58(key_hex: str, prefix: int = 42) -> str:
    """Generic SS58 (the node deserializes GRANDPA AuthorityIds from SS58)."""
    key = bytes.fromhex(key_hex.removeprefix("0x"))
    scale.require(len(key) == 32, "GRANDPA key must be 32 bytes")
    body = bytes([prefix]) + key
    data = body + hashlib.blake2b(b"SS58PRE" + body, digest_size=64).digest()[:2]
    number, text = int.from_bytes(data, "big"), ""
    while number:
        number, remainder = divmod(number, 58)
        text = B58[remainder] + text
    return "1" * (len(data) - len(data.lstrip(b"\0"))) + text


def prepare(build: Path, output: Path) -> dict:
    manifest = json.loads((build / "BUILD-MANIFEST.json").read_bytes())
    scale.require(manifest.get("status") == "passed", "production build did not pass")
    policy = (build / "policy.scale").read_bytes()
    identity = release.policy_identity(policy)
    consensus = json.loads((build / "checkpoint-consensus" / "stdout.log").read_bytes())
    public = json.loads((build / "public-consensus.json").read_bytes())
    code = {}
    for role, directory, version in (("bootstrap", "build-134", 134), ("hold", "build-135", 135),
                                     ("final", "build-final138", 138)):
        data = (build / directory / f"runtime-{version}.compact.compressed.wasm").read_bytes()
        key = "final138" if role == "final" else str(version)
        scale.require(manifest["artifacts"][key]["blake2_256"] == blake(data)[2:], f"{role} Wasm differs from build manifest")
        code[role] = blake(data)
    binding = json.loads((build / "bindings" / "binding-134.json").read_bytes())
    fields = public.get("fields", public)
    first, last = fields["firstRecoverySlot"], fields["lastObservedAbandonedSlot"]
    scale.require(last == 298_871_044 and first == last + 1 and binding["firstRecoverySlot"] == first,
                  "public slot bounds differ from the bootstrap binding")
    scale.require(consensus["previousGrandpaSetId"] == 7956 and binding["previousGrandpaSetId"] == 7956,
                  "checkpoint GRANDPA set differs")
    scale.require(consensus["grandpaAuthorities"] == consensus["previousGrandpaAuthorities"],
                  "retained GRANDPA authorities must equal the checkpoint set")
    approved = sorted(consensus["approvedGrandpaKeys"])
    scale.require(approved == consensus["approvedGrandpaKeys"], "approved GRANDPA keys must be sorted")
    config = {
        "schemaVersion": 3,
        "checkpointHash": scale.CHECKPOINT_HASH,
        "policyCommitment": "0x" + identity["commitment"],
        "policyScale": "0x" + policy.hex(),
        "bootstrapCodeHash": code["bootstrap"],
        "finalCodeHash": code["final"],
        "holdCodeHash": code["hold"],
        "evidenceCommitment": "0x" + identity["evidenceCommitment"],
        "previousGrandpaSetId": 7956,
        "previousGrandpaAuthorities": [[ss58(key), weight] for key, weight in consensus["previousGrandpaAuthorities"]],
        "grandpaAuthorities": [[ss58(key), weight] for key, weight in consensus["grandpaAuthorities"]],
        "approvedGrandpaKeys": [ss58(key) for key in approved],
        "lastObservedAbandonedSlot": last,
        "firstRecoverySlot": first,
    }
    raw = (json.dumps(config, indent=2) + "\n").encode()
    with output.open("xb") as stream:
        stream.write(raw)
    return {"output": str(output), "sha256": hashlib.sha256(raw).hexdigest(), "policyCommitment": config["policyCommitment"],
            "bootstrapCodeHash": code["bootstrap"], "holdCodeHash": code["hold"], "finalCodeHash": code["final"],
            "authorities": len(config["grandpaAuthorities"]), "approvedKeys": len(approved), "productionReady": False}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(prepare(args.build, args.output), indent=2))
