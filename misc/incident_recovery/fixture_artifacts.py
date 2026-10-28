#!/usr/bin/env python3
"""Prepare isolated, explicitly non-production SORA Nexus recovery fixtures.

No network, operator credentials, deployment, publication or production approvals.
The deterministic GRANDPA fixture keys are publicly derivable test keys. They must
never be used to sign on a live network. The exporter preserves the working tree.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

import hashi_codec as scale
import release_package as release
from bootstrap.prepare import ROOT, SOURCE_PATHS

FIXTURE = "SORA Nexus PUBLIC FIXTURE ONLY — NEVER DEPLOY"


def digest(data):
    return {"sha256": hashlib.sha256(data).hexdigest(), "blake2_256": hashlib.blake2b(data, digest_size=32).hexdigest()}


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def fresh_directory(path):
    scale.require(not path.exists(), "Fixture output must be a new directory; existing build lanes are never overwritten")
    path.mkdir(parents=True)


def policy_bytes(path):
    data = path.read_bytes()
    if data.lstrip().startswith(b"{"):
        document = json.loads(data)
        data = scale.raw(document["work"]["policyScale"])
    identity = release.policy_identity(data)
    return data, identity


def final_source(policy):
    identity = release.policy_identity(policy)
    array = lambda raw: "[" + ", ".join(str(b) for b in raw) + "]"
    replacements = "".join(f"    ({array(bytes.fromhex(old))}, {array(bytes.fromhex(new))}),\n" for old, new in identity["mapping"])
    return (f"//! {FIXTURE}. No production ownership or signing-stop approval.\n"
            f"pub const POLICY_SCALE: &[u8] = &{array(policy)};\n"
            f"pub const POLICY_COMMITMENT: [u8; 32] = {array(bytes.fromhex(identity['commitment']))};\n"
            f"pub const REPLACEMENTS: &[([u8; 32], [u8; 32])] = &[\n{replacements}];\n")


def export_final(policy_path, destination, source_root=ROOT):
    policy, identity = policy_bytes(policy_path)
    source = (source_root / "runtime/src/lib.rs").read_text()
    scale.require(re.search(r"spec_version:\s*138,", source) and re.search(r"transaction_version:\s*134,", source), "Final fixture requires runtime138/transaction134 source")
    names = sorted(set(filter(None, subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", *SOURCE_PATHS], cwd=source_root).decode().split("\0"))))
    scale.require(names and "Cargo.lock" in names and "runtime/src/incident_recovery/generated.rs" in names, "Incomplete final source export")
    fresh_directory(destination)
    sources = {}
    for name in names:
        relative = Path(name)
        path = source_root / relative
        scale.require(not relative.is_absolute() and ".." not in relative.parts and path.is_file() and not path.is_symlink(), f"Unsafe source entry: {name}")
        payload = path.read_bytes()
        output = destination / relative
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(payload)
        shutil.copymode(path, output)
        sources[name] = digest(payload)
    # Reject a concurrent source change; do not certify a mixed export. Preserve
    # the failed output for inspection rather than silently replacing a lane.
    for name, expected in sources.items():
        scale.require(digest((source_root / name).read_bytes()) == expected, f"Source changed during fixture export: {name}")
    generated = destination / "runtime/src/incident_recovery/generated.rs"
    generated.write_text(final_source(policy))
    (destination / "fixture-policy.scale").write_bytes(policy)
    manifest = {"schema": "sora-nexus-final-fixture-source-v1", "warning": FIXTURE,
                "fixtureOnly": True, "productionReady": False, "specVersion": 138, "transactionVersion": 134,
                "sourceHead": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source_root, text=True).strip(),
                "policy": digest(policy), "policyIdentity": identity, "sourceFiles": sources,
                "generatedPolicy": digest(generated.read_bytes()), "exporter": digest(Path(__file__).read_bytes())}
    write_json(destination / "FIXTURE-PROVENANCE.json", manifest)
    (destination / "NEVER-DEPLOY.txt").write_text(FIXTURE + "\n")
    return manifest


def fixture_operators(inventory):
    scale.require(inventory.get("schema") in ("sora-nexus-checkpoint-consensus-membership-v2", "sora-nexus-checkpoint-consensus-membership-v3"), "Observed signing inventory v2/v3 required")
    scale.require(inventory.get("checkpointHash") == scale.CHECKPOINT_HASH and inventory.get("stateRoot") == release.CHECKPOINT_ROOT, "Wrong fixture checkpoint")
    rows = inventory["observedSigningInventory"]
    scale.require(0 < len(rows) <= 1000 and [r["owner"] for r in rows] == inventory["validators"], "Incomplete or reordered fixture operator inventory")
    # Construct deterministic test-only Ed25519 public keys with the built-in
    # Node crypto implementation. No private material is read or written.
    js = r'''
const {createHash,createPrivateKey,createPublicKey}=require('node:crypto');
let input=''; process.stdin.on('data',c=>input+=c); process.stdin.on('end',()=>{
  const keys=JSON.parse(input).map(owner=>{
    const seed=createHash('sha256').update('sora-nexus/PUBLIC-FIXTURE-ONLY/grandpa/v1/').update(owner).digest();
    const key=createPrivateKey({key:Buffer.concat([Buffer.from('302e020100300506032b657004220420','hex'),seed]),format:'der',type:'pkcs8'});
    return '0x'+createPublicKey(key).export({format:'der',type:'spki'}).subarray(-32).toString('hex');
  }); process.stdout.write(JSON.stringify(keys));
});'''
    fresh = json.loads(subprocess.check_output(["node", "-e", js], input=json.dumps([r["owner"] for r in rows]).encode()))
    encoded = bytearray(scale.compact(len(rows)))
    output = []
    for row, key in zip(rows, fresh, strict=True):
        encoded += scale.raw(row["owner"], 32, True)
        unions = {}
        for label in ("retiredGrandpa", "retainedBabe", "retainedImOnline"):
            values = [scale.raw(value, 32, True) for value in row[label]]
            scale.require(0 < len(values) <= 3 and values == sorted(set(values)), f"Invalid observed {label} union")
            unions[label] = scale.compact(len(values)) + b"".join(values)
        encoded += unions["retiredGrandpa"] + scale.raw(key, 32, True) + unions["retainedBabe"] + unions["retainedImOnline"]
        output.append({**row, "freshGrandpa": key, "operatorApproved": False, "fixtureOnly": True})
    release.operator_identity(bytes(encoded))
    return bytes(encoded), output


def write_operators(inventory_path, destination):
    raw = inventory_path.read_bytes()
    encoded, rows = fixture_operators(json.loads(raw))
    fresh_directory(destination)
    (destination / "operators.scale").write_bytes(encoded)
    write_json(destination / "operators.json", rows)
    manifest = {"schema": "sora-nexus-public-fixture-operators-v1", "warning": FIXTURE, "fixtureOnly": True,
                "productionReady": False, "inventory": digest(raw), "operators": digest(encoded),
                "signingStopVerified": False, "operatorApprovalsVerified": False}
    write_json(destination / "FIXTURE-PROVENANCE.json", manifest)
    return manifest


def write_bindings(policy_path, operators_path, final_path, hold_path, previous_set, first_slot, destination):
    policy, identity = policy_bytes(policy_path)
    operators = operators_path.read_bytes()
    release.operator_identity(operators)
    final = final_path.read_bytes()
    version = release.wasm_identity(final)
    scale.require((version["specVersion"], version["transactionVersion"]) == (138, 134), "Wrong final fixture identity")
    scale.integer(previous_set, 2**64 - 2)
    scale.require(0 < scale.integer(first_slot, 2**64 - 1), "Invalid first fixture slot")
    hold = hold_path.read_bytes() if hold_path else None
    if hold is not None:
        version = release.wasm_identity(hold)
        scale.require((version["specVersion"], version["transactionVersion"]) == (135, 132), "Wrong hold fixture identity")
    fresh_directory(destination)
    (destination / "policy.scale").write_bytes(policy)
    (destination / "operators.scale").write_bytes(operators)
    (destination / "final-138.wasm").write_bytes(final)
    binding = {"policyScale": "0x" + policy.hex(), "policyCommitment": "0x" + identity["commitment"],
               "operatorsScale": "0x" + operators.hex(), "previousGrandpaSetId": previous_set,
               "firstRecoverySlot": first_slot, "finalWasm": "final-138.wasm"}
    write_json(destination / "hold-135-binding.json", binding)
    if hold is not None:
        (destination / "hold-135.wasm").write_bytes(hold)
        write_json(destination / "bootstrap-134-binding.json", {**binding, "holdWasm": "hold-135.wasm"})
    manifest = {"schema": "sora-nexus-public-fixture-bindings-v1", "warning": FIXTURE, "fixtureOnly": True,
                "productionReady": False, "policy": digest(policy), "operators": digest(operators),
                "final": digest(final), "hold": digest(hold) if hold else None,
                "signingStopVerified": False, "operatorApprovalsVerified": False,
                "firstSlotMeaning": "Local execution fixture only; not a claim about abandoned-chain signing"}
    write_json(destination / "FIXTURE-PROVENANCE.json", manifest)
    (destination / "NEVER-DEPLOY.txt").write_text(FIXTURE + "\n")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    final = sub.add_parser("final-source")
    final.add_argument("--policy", type=Path, required=True)
    final.add_argument("--destination", type=Path, required=True)
    operators = sub.add_parser("operators")
    operators.add_argument("--inventory", type=Path, required=True)
    operators.add_argument("--destination", type=Path, required=True)
    bindings = sub.add_parser("bindings")
    for name in ("policy", "operators", "final", "destination"):
        bindings.add_argument("--" + name, type=Path, required=True)
    bindings.add_argument("--hold", type=Path)
    bindings.add_argument("--previous-set", type=int, required=True)
    bindings.add_argument("--first-slot", type=int, required=True)
    args = parser.parse_args()
    try:
        if args.command == "final-source":
            report = export_final(args.policy, args.destination.resolve())
        elif args.command == "operators":
            report = write_operators(args.inventory, args.destination.resolve())
        else:
            report = write_bindings(args.policy, args.operators, args.final, args.hold, args.previous_set, args.first_slot, args.destination.resolve())
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Fixture preparation rejected: {error}\n")
    print(json.dumps({"destination": str(args.destination.resolve()), "schema": report["schema"], "fixtureOnly": True, "productionReady": False}))


if __name__ == "__main__":
    main()
