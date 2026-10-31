#!/usr/bin/env python3
"""Prepare a public test-council escape-only rehearsal; never production governance.

The fixed test key derivation is public. No production keys are read or written.
Signatures are produced by the native harness from actual persisted execution
state. This tool never invents a cursor, counter or executing code hash.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

import fixture_artifacts as fixture
import hashi_codec as scale
import release_package as release

SCHEME = "sora-nexus/PUBLIC-FIXTURE-ONLY/council/v1/"
PROBE_KEY = b":sora-nexus:public-fixture:escape-probe:v1"
PROBE_VALUE = b"verified-forward-repair-probe"
MANIFEST_SCHEMA = "sora-nexus-public-fixture-escape-replay-v1"


def council():
    js = r'''
const {createHash,createPrivateKey,createPublicKey}=require('node:crypto');
const prefix=process.argv[1];
console.log(JSON.stringify([0,1,2].map(index=>{
 const seed=createHash('sha256').update(prefix+index).digest();
 const key=createPrivateKey({key:Buffer.concat([Buffer.from('302e020100300506032b657004220420','hex'),seed]),format:'der',type:'pkcs8'});
 return createPublicKey(key).export({format:'der',type:'spki'}).subarray(-32).toString('hex');
}).sort()));'''
    return [bytes.fromhex(value) for value in json.loads(subprocess.check_output(["node", "-e", js, SCHEME]))]


def hash_bytes(value):
    return hashlib.blake2b(value, digest_size=32).digest()


def chunk(failing):
    before = b"\x01" + hash_bytes(b"intentionally nonexistent public fixture preimage") if failing else b"\x00"
    edit = b"\x00" + scale.vec(PROBE_KEY) + before + b"\x01" + scale.vec(PROBE_VALUE)
    return scale.u32(0) + scale.compact(1) + edit


def audit():
    return scale.u32(0) + scale.compact(1) + b"\x00" + scale.vec(PROBE_KEY) + b"\x01" + hash_bytes(PROBE_VALUE)


def policy(keys, failing):
    result = (scale.u32(2) + scale.raw(scale.CHECKPOINT_HASH) + scale.raw(release.CHECKPOINT_ROOT) + scale.compact(0)
              + scale.compact(len(keys)) + b"".join(keys) + scale.compact(1) + hash_bytes(chunk(failing))
              + scale.compact(1) + hash_bytes(audit())
              + release.authorization_domain_scale({"chainSpecId": "sora-nexus-PUBLIC-FIXTURE-ONLY", "releaseNonce": "0x" + (bytes([0x46]) * 32).hex()})
              + hash_bytes(b"SORA Nexus PUBLIC FIXTURE ONLY; observed governance unchanged; " + (b"failing" if failing else b"repaired"))
              + hash_bytes(b"SORA Nexus escape-only fixture; no terminal audit or final activation"))
    release.policy_identity(result)
    return result


def inputs(destination, operators_path, final_path, previous_set, first_slot):
    operators, final = operators_path.read_bytes(), final_path.read_bytes()
    release.operator_identity(operators)
    identity = release.wasm_identity(final)
    scale.require((identity["specVersion"], identity["transactionVersion"]) == (138, 134), "Wrong unused final artifact identity")
    scale.integer(previous_set, 2**64 - 2)
    scale.require(0 < scale.integer(first_slot, 2**64 - 1), "Invalid fixture first slot")
    keys = council()
    original, revised = policy(keys, True), policy(keys, False)
    fixture.fresh_directory(destination)
    values = {"original-policy.scale": original, "revised-policy.scale": revised, "operators.scale": operators,
              "unused-final-138.wasm": final, "failing-apply.scale": b"snxrec01\x01\x01" + chunk(True),
              "repaired-apply.scale": b"snxrec01\x01\x01" + chunk(False), "repaired-audit.scale": b"snxrec01\x01\x02" + audit()}
    for name, data in values.items():
        (destination / name).write_bytes(data)
    binding = {"policyScale": "0x" + original.hex(), "policyCommitment": "0x" + hash_bytes(original).hex(),
               "operatorsScale": "0x" + operators.hex(), "previousGrandpaSetId": previous_set,
               "firstRecoverySlot": first_slot, "finalWasm": "unused-final-138.wasm"}
    fixture.write_json(destination / "hold-135-binding.json", binding)
    fixture.write_json(destination / "repair-136-binding.json", {**binding, "policyScale": "0x" + revised.hex(),
                       "policyCommitment": "0x" + hash_bytes(revised).hex(), "originalPolicyScale": "0x" + original.hex(),
                       "originalPolicyCommitment": "0x" + hash_bytes(original).hex()})
    provenance = {"schema": "sora-nexus-public-fixture-escape-inputs-v1", "fixtureOnly": True, "productionReady": False,
                  "warning": fixture.FIXTURE, "purpose": "escape-only", "councilScheme": SCHEME,
                  "publicCouncil": ["0x" + key.hex() for key in keys], "governanceStateReplaced": False,
                  "automaticActivationAllowed": False, "finalArtifactMeaning": "Version-checked unused artifact; its policy is not this test council policy",
                  "files": {name: fixture.digest(data) for name, data in values.items()},
                  "firstSlotMeaning": "Local execution only; no operator signing-stop approval"}
    fixture.write_json(destination / "FIXTURE-PROVENANCE.json", provenance)
    (destination / "NEVER-DEPLOY.txt").write_text(fixture.FIXTURE + "\n")
    return provenance


def bootstrap_binding(inputs_path, hold_path, destination):
    original = inputs_path / "original-policy.scale"
    base = json.loads((inputs_path / "hold-135-binding.json").read_text())
    fixture.write_bindings(original, inputs_path / "operators.scale", inputs_path / "unused-final-138.wasm", hold_path,
                           base["previousGrandpaSetId"], base["firstRecoverySlot"], destination)


def manifest(root, destination, inputs_path, consensus_path, operators_path, bootstrap, hold, repair, final):
    root, destination = root.resolve(), destination.resolve()
    scale.require(destination.is_relative_to(root) and destination != root and not destination.exists(), "New fixture destination inside artifact root required")
    provenance = scale.read_json(inputs_path / "FIXTURE-PROVENANCE.json")
    scale.require(provenance.get("schema") == "sora-nexus-public-fixture-escape-inputs-v1" and provenance.get("purpose") == "escape-only", "Escape fixture source required")
    for name, expected in provenance["files"].items():
        scale.require(fixture.digest((inputs_path / name).read_bytes()) == expected, "Changed escape fixture input")
    consensus, operators = scale.read_json(consensus_path), scale.read_json(operators_path)
    scale.require(consensus["checkpointHash"] == scale.CHECKPOINT_HASH and consensus["stateRoot"] == release.CHECKPOINT_ROOT, "Wrong fixture checkpoint")
    active = fixture.fixture_active_grandpa(consensus, operators)
    destination.mkdir(parents=True)

    def artifact(name, data):
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("xb") as stream:
            stream.write(data)
        return {"file": path.relative_to(root).as_posix(), "sha256": hashlib.sha256(data).hexdigest()}

    runtime = {}
    for role, source, pair in (("bootstrap", bootstrap, (134, 131)), ("hold", hold, (135, 132)), ("repair", repair, (136, 132)), ("finalRuntime", final, (138, 134))):
        data = source.read_bytes()
        version = release.wasm_identity(data)
        scale.require((version["specVersion"], version["transactionVersion"]) == pair, "Wrong escape runtime identity")
        runtime[role] = artifact(f"{role}-{pair[0]}.wasm", data)
    original = artifact("original-policy.scale", (inputs_path / "original-policy.scale").read_bytes())
    revised = artifact("revised-policy.scale", (inputs_path / "revised-policy.scale").read_bytes())
    public_provenance = artifact("FIXTURE-COUNCIL.json", (inputs_path / "FIXTURE-PROVENANCE.json").read_bytes())
    slot, timestamp = int(consensus["checkpointSlot"]), int(consensus["checkpointTimestamp"])
    scale.require(timestamp // 6000 == slot, "Wrong checkpoint slot timestamp")
    timestamp_offset, steps = timestamp % 6000, []

    def step(name, payload, phase, spec, escape=None, reject_prior=False):
        nonlocal slot
        slot += 1
        steps.append({"slot": slot, "timestamp": slot * 6000 + timestamp_offset, "authorityIndex": 0,
                      "recovery": artifact(f"work/{name}.scale", payload) if payload is not None else None,
                      "fixtureEscape": escape, "rejectPreviousEscapes": reject_prior,
                      "expectedSpec": spec, "expectedPhase": phase})

    step("failed", (inputs_path / "failing-apply.scale").read_bytes(), "Failed", 134)
    step("hold", None, "Hold", 135, {"destinationSpec": 135, "workPolicy": None})
    repair_bytes = repair.read_bytes()
    for index, chunk_offset in enumerate(range(0, len(repair_bytes), 65536)):
        payload = b"snxrec01\x01\x03\x02" + scale.u32(index) + scale.vec(repair_bytes[chunk_offset:chunk_offset + 65536])
        step(f"repair-code-{index:05}", payload, "Hold", 135, reject_prior=index == 0)
    step("forward", None, "Applying", 136, {"destinationSpec": 136, "workPolicy": revised}, reject_prior=True)
    step("repaired", (inputs_path / "repaired-apply.scale").read_bytes(), "Auditing", 136, reject_prior=True)
    step("audited", (inputs_path / "repaired-audit.scale").read_bytes(), "Auditing", 136, reject_prior=True)
    document = {"schema": MANIFEST_SCHEMA, "checkpointHash": scale.CHECKPOINT_HASH, "checkpointCapture": "checkpoint",
                "policy": original, "bootstrap": runtime["bootstrap"], "hold": runtime["hold"], "finalRuntime": runtime["finalRuntime"],
                "repairs": [runtime["repair"]], "previousGrandpaSet": int(consensus["previousGrandpaSet"]),
                "freshGrandpa": active,
                "fixtureCouncil": {"scheme": SCHEME, "provenance": public_provenance}, "steps": steps}
    output = root / (destination.relative_to(root).as_posix().replace("/", "-") + ".json")
    with output.open("x") as stream:
        json.dump(document, stream, indent=2)
        stream.write("\n")
    result = {"schema": "sora-nexus-public-fixture-escape-manifest-v1", "fixtureOnly": True, "productionReady": False,
              "manifest": str(output), "manifestSha256": hashlib.sha256(output.read_bytes()).hexdigest(), "blocks": len(steps),
              "automaticActivationAllowed": False, "governanceStateReplaced": False,
              "signatures": "Generated by exact-Wasm harness from actual execution state; no precomputed authorization hashes"}
    fixture.write_json(destination / "FIXTURE-PROVENANCE.json", result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    initial = commands.add_parser("inputs")
    for name in ("destination", "operators", "final"):
        initial.add_argument("--" + name, type=Path, required=True)
    initial.add_argument("--previous-set", type=int, required=True)
    initial.add_argument("--first-slot", type=int, required=True)
    boot = commands.add_parser("bootstrap-binding")
    for name in ("inputs", "hold", "destination"):
        boot.add_argument("--" + name, type=Path, required=True)
    plan = commands.add_parser("manifest")
    for name in ("root", "destination", "inputs", "consensus", "operators", "bootstrap", "hold", "repair", "final"):
        plan.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    if args.command == "inputs":
        report = inputs(args.destination.resolve(), args.operators, args.final, args.previous_set, args.first_slot)
    elif args.command == "bootstrap-binding":
        bootstrap_binding(args.inputs, args.hold, args.destination.resolve())
        report = {"fixtureOnly": True, "destination": str(args.destination.resolve())}
    else:
        report = manifest(args.root, args.destination, args.inputs, args.consensus, args.operators, args.bootstrap, args.hold, args.repair, args.final)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
