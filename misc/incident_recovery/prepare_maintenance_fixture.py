#!/usr/bin/env python3
"""Bind public diagnostic Wasm and committed work to an offline replay manifest.

No production approvals, private keys, RPC calls, node DB changes or publication.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path

import hashi_codec as scale
import release_package as release
import fixture_work_transport as transport

MIXED_SCHEMA = "sora-nexus-PUBLIC-FIXTURE-ONLY-mixed-bridge-v2"
HASHI_SCHEMA = "sora-nexus-PUBLIC-FIXTURE-ONLY-hashi-bridge-v3"
BRIDGE_HALT_KEY = "0x" + b":sora-nexus:recovery:bridge-halt:v1".hex()
BRIDGE_HALT_VALUE_HASH = "0x" + hashlib.blake2b(b"\x01", digest_size=32).hexdigest()


def storage_records(rows):
    """Validate the native export's canonical bounded key/value-hash inventory."""
    scale.require(isinstance(rows, list) and 0 < len(rows) <= 4096,
                  "Complete native storage records required")
    result = {}
    for row in rows:
        scale.require(isinstance(row, dict) and set(row) == {"key", "valueHash"},
                      "Invalid native storage record")
        key = scale.raw(row["key"])
        scale.require(key and row["key"] not in result, "Duplicate or empty native storage key")
        if row["valueHash"] is not None:
            scale.raw(row["valueHash"], 32)
        result[row["key"]] = row["valueHash"]
    scale.require(list(result) == sorted(result), "Native storage records must be ordered")
    return result


def retained_configuration(document, expectations, hashi):
    # These are hashes exported from the native execution. This adapter binds
    # them into replay expectations; it does not certify native execution itself.
    scale.require(document.get("nativeBridgeHaltVerified") is True and
                  document.get("publicConfigurationRetained") is True and
                  document.get("cleanupStorageEditsEmitted") is False,
                  "Native halted retained public configuration required")
    retained = storage_records(document.get("retainedPublicConfiguration"))
    final = storage_records(expectations["records"])
    scale.require(type(document.get("retainedConfigurationKeys")) is int and
                  document["retainedConfigurationKeys"] == len(retained) and
                  (not hashi or len(retained) == 6) and
                  all(key in final and final[key] == value for key, value in retained.items()),
                  "Retained public configuration must match native final records")
    scale.require(final.get(BRIDGE_HALT_KEY) == BRIDGE_HALT_VALUE_HASH,
                  "Native final records must preserve the persistent bridge halt")
    for operation in expectations["operations"]:
        storage_records(operation["records"])


def work_inputs(document):
    """Keep structural assembly and native preview evidence distinct."""
    scale.require(document.get("productionReady") is False, "Diagnostic work required")
    work = document["work"]
    schema = document.get("schema")
    expectations = None
    if schema == "sora-nexus-bridge-cancellation-rehearsal-v1":
        scale.require(document.get("historicalCallsExecuted") is False and
                      work.get("storageEngineRehearsed") is True,
                      "Verified diagnostic cancellation rehearsal required")
        envelopes = work["applyEnvelopes"]
    elif schema == "sora-nexus-PUBLIC-FIXTURE-ONLY-storage-v1":
        scale.require(document.get("fixtureOnly") is True and
                      document.get("victimAuthorizationVerified") is False and
                      document.get("exactWasmRehearsed") is False and
                      work.get("schema") == "sora-nexus-storage-work-v1" and
                      work.get("productionReady") is False and
                      work.get("storageEngineRehearsed") is True,
                      "Explicit public storage fixture and completed native composition required")
        envelopes = work["applyEnvelopes"]
    elif schema in (MIXED_SCHEMA, HASHI_SCHEMA):
        scale.require(document.get("fixtureOnly") is True and
                      document.get("nativePreviewCompleted") is True and
                      document.get("exactWasmRehearsed") is False and
                      len(document.get("nativePreviewEvidence", [])) == 2,
                      "Two native public fixture previews required")
        scale.require(work.get("schema") == "sora-nexus-structural-mixed-work-v1" and
                      work.get("structuralValidationOnly") is True and
                      all(work.get(flag) is False for flag in (
                          "authenticationVerified", "economicCompletionVerified",
                          "storageEngineRehearsed", "exactArtifactRehearsed", "productionReady")),
                      "Structural assembly must not be promoted to execution evidence")
        envelopes = work["workEnvelopes"]
        # Native previews bind block-number-dependent records to these three
        # (mixed) or four (HASHI) children. Public configuration remains halted;
        # no raw cleanup Apply is permitted after authenticated bridge work.
        hashi = schema == HASHI_SCHEMA
        schedule = [(0, "storage"), (1, "incoming"), (2, "incoming"), (3, "incoming")] if hashi else [
            (0, "storage"), (1, "incoming"), (2, "outgoing")]
        scale.require([(row["index"], row["kind"]) for row in work["work"]] == schedule and
                      len(envelopes) == len(schedule), "Unexpected public bridge fixture work schedule")
        expectations = document["fixtureExpectations"]
        scale.require(expectations.get("schema") == "sora-nexus-public-fixture-storage-expectations-v1" and
                      expectations.get("fixtureOnly") is True and expectations.get("records") and
                      expectations.get("policyCommitment") == work["policyCommitment"] and
                      [row["index"] for row in expectations.get("operations", [])] == ([1, 2, 3] if hashi else [1, 2]),
                      "Complete policy-bound dynamic storage expectations required")
        retained_configuration(document, expectations, hashi)
        if hashi:
            scale.require(document.get("nativeZeroSenderPreserved") is True and
                          document.get("incomingScenarios") == ["retained-pending-credit", "fresh-credit", "fresh-source-actor-exclusion"] and
                          document.get("signatureChainDomain") == "0x01" and
                          document.get("sourceActor") == "0x9d8a62f656a8d1615c1294fd71e9cfb3e4855a4f" and
                          document.get("excludedSourceActor") == "0xb595b18c88b1f651ca387489067f855b5c8e6720" and
                          document.get("identicalSourceRegistryAcrossOperations") is True and
                          document.get("minted") == "2",
                          "Explicit zero-sender source-actor fixture scope required")
        if hashi:
            provenance = document.get("fixtureHashiProvenance", {})
            scale.require(provenance.get("schema") == "sora-nexus-public-fixture-hashi-inputs-v1" and
                          provenance.get("fixtureOnly") is True and provenance.get("productionReady") is False and
                          provenance.get("checkpointHash") == scale.CHECKPOINT_HASH and
                          provenance.get("policyCommitment") == work["policyCommitment"] and
                          provenance.get("peerScheme") == "sora-nexus/PUBLIC-FIXTURE-ONLY/hashi-peer/v1/" and
                          len(provenance.get("publicPeers", [])) == 4,
                          "Explicit policy-bound public HASHI peer provenance required")
    else:
        raise ValueError("Unsupported diagnostic work schema")
    scale.require(envelopes and work["auditEnvelopes"], "Incomplete fixture work")
    return work, envelopes, expectations


def prepare(root, destination, work_path, consensus_path, operators_path, bootstrap, hold, final):
    root, destination = root.resolve(), destination.resolve()
    scale.require(destination.is_relative_to(root) and destination != root and not destination.exists(), "Fixture destination must be new and inside artifact root")
    scale.require((root / "checkpoint/capture-complete.json").is_file(), "Missing complete checkpoint capture")
    work_document = scale.read_json(work_path)
    if work_document.get("schema") == transport.SCHEMA:
        work_document = transport.load(work_path, work_document)
    work, envelopes, expectations = work_inputs(work_document)
    policy = scale.raw(work["policyScale"])
    identity = release.policy_identity(policy)
    scale.require(identity["commitment"] == work["policyCommitment"].removeprefix("0x"), "Policy commitment mismatch")
    consensus, operators = scale.read_json(consensus_path), scale.read_json(operators_path)
    scale.require(consensus["checkpointHash"] == scale.CHECKPOINT_HASH and consensus["stateRoot"] == release.CHECKPOINT_ROOT,
                  "Wrong consensus checkpoint")
    scale.require([row["owner"] for row in operators] == consensus["validators"] and
                  all(row["fixtureOnly"] and not row["operatorApproved"] for row in operators), "Public fixture operators required")
    active = next(row["value"] for row in consensus["records"] if row["storage"] == "Grandpa.Authorities")
    scale.require(len(active) == len(operators), "Active GRANDPA weight inventory mismatch")
    destination.mkdir(parents=True)

    def artifact(name, data):
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("xb") as stream:
            stream.write(data)
        return {"file": target.relative_to(root).as_posix(), "sha256": hashlib.sha256(data).hexdigest()}

    wasm = {}
    for role, source, version in (("bootstrap", bootstrap, (134, 131)), ("hold", hold, (135, 132)), ("finalRuntime", final, (138, 134))):
        data = source.read_bytes()
        actual = release.wasm_identity(data)
        scale.require((actual["specVersion"], actual["transactionVersion"]) == version, "Wrong fixture Wasm identity")
        wasm[role] = artifact(f"{role}-{version[0]}.wasm", data)
    final_bytes = (root / wasm["finalRuntime"]["file"]).read_bytes()
    slot = int(consensus["checkpointSlot"])
    checkpoint_time = int(consensus["checkpointTimestamp"])
    scale.require(checkpoint_time // 6000 == slot, "Checkpoint timestamp/slot configuration mismatch")
    timestamp_offset = checkpoint_time % 6000
    steps = []
    malformed = artifact("work/malformed.scale", b"snxrec01\x01\xfe")

    def step(name, data, phase, expected_spec=134, rejected=()):
        nonlocal slot
        # The retained runtime has 600 six-second slots per epoch. The second
        # fixture block spans an epoch while the economic session stays frozen.
        slot += 600 if len(steps) == 1 else 1
        payload = artifact(f"work/{name}.scale", data) if data is not None else None
        steps.append({"slot": slot, "timestamp": slot * 6000 + timestamp_offset, "authorityIndex": 0,
                      "recovery": payload, "rejectedRecovery": list(rejected),
                      "expectedSpec": expected_spec, "expectedPhase": phase})
        return payload

    previous = None
    for index, payload in enumerate(envelopes):
        previous = step(f"apply-{index:05}", scale.raw(payload),
                        "Auditing" if index + 1 == len(envelopes) else "Applying",
                        rejected=[malformed] + ([previous] if previous else []))
        if work_document["schema"] == HASHI_SCHEMA and index in (1, 2, 3):
            steps[-1]["fixtureHashi"] = True
    for index, payload in enumerate(work["auditEnvelopes"]):
        step(f"audit-{index:05}", scale.raw(payload), "Auditing")
    previous = None
    for index, offset in enumerate(range(0, len(final_bytes), 65536)):
        chunk = final_bytes[offset:offset + 65536]
        # RecoveryInherent v1, RecoveryAction::StageCode(3), Final(0).
        payload = b"snxrec01\x01\x03\x00" + scale.u32(index) + scale.vec(chunk)
        previous = step(f"final-code-{index:05}", payload, "Auditing", rejected=[previous] if previous else [])
    for index, payload in enumerate(work.get("terminalPreludeEnvelopes", [])):
        step(f"terminal-page-{index:05}", scale.raw(payload), "Auditing")
    step("terminal", scale.raw(work["terminalEnvelope"]), "Complete", expected_spec=138)
    slot += 600
    step("ordinary", None, "Complete", expected_spec=138)
    step("ordinary-following", None, "Complete", expected_spec=138)
    manifest = {"schema": "sora-nexus-maintenance-replay-v1", "checkpointHash": scale.CHECKPOINT_HASH,
                "checkpointCapture": "checkpoint", "policy": artifact("policy.scale", policy), **wasm,
                "repairs": [], "previousGrandpaSet": int(consensus["previousGrandpaSet"]),
                "freshGrandpa": [[row["freshGrandpa"], int(entry[1])] for row, entry in zip(operators, active, strict=True)],
                "steps": steps}
    if expectations is not None:
        manifest["fixtureExpectations"] = artifact("storage-expectations.json",
            (json.dumps(expectations, indent=2) + "\n").encode())
    if work_document["schema"] == HASHI_SCHEMA:
        provenance = work_document["fixtureHashiProvenance"]
        manifest["schema"] = "sora-nexus-public-fixture-hashi-replay-v1"
        manifest["fixtureHashiPeers"] = {"scheme": provenance["peerScheme"], "provenance": artifact(
            "public-hashi-provenance.json", (json.dumps(provenance, indent=2) + "\n").encode())}
    name = destination.relative_to(root).as_posix().replace("/", "-") + ".json"
    manifest_path = root / name
    with manifest_path.open("x") as stream:
        json.dump(manifest, stream, indent=2)
        stream.write("\n")
    provenance = {"schema": "sora-nexus-public-fixture-maintenance-v1", "fixtureOnly": True, "productionReady": False,
                  "warning": "PUBLIC DIAGNOSTIC FIXTURE ONLY — NEVER DEPLOY", "manifest": manifest_path.name,
                  "manifestSha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
                  "workReportSha256": hashlib.sha256(work_path.read_bytes()).hexdigest(),
                  "consensusObservationSha256": hashlib.sha256(consensus_path.read_bytes()).hexdigest(),
                  "operatorFixtureSha256": hashlib.sha256(operators_path.read_bytes()).hexdigest(),
                  "operatorApprovalVerified": False, "signingStopVerified": False, "blocks": len(steps),
                  "workSchema": work_document["schema"], "exactWasmRehearsed": False}
    if "transportSchema" in work_document:
        provenance["workTransportSchema"] = work_document["transportSchema"]
        provenance["compiledWorkSha256"] = work_document["compiledWorkSha256"]
    artifact("FIXTURE-PROVENANCE.json", (json.dumps(provenance, indent=2) + "\n").encode())
    return provenance


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("root", "destination", "work", "consensus", "operators", "bootstrap", "hold", "final"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(prepare(args.root, args.destination, args.work, args.consensus, args.operators,
                             args.bootstrap, args.hold, args.final), indent=2))
