#!/usr/bin/env python3
"""Content-addressed, fail-closed SORA Nexus recovery release preparation.

Static artifact consistency is not production readiness. Source reports cannot
clear a gate. This version has fixed independent evidence-verifier hooks
but no complete exact-artifact consensus/rehearsal verifier; consequently
it always emits productionReady=false and has no override to promote a package.
"""
from __future__ import annotations

import argparse
import hashlib
import gzip
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import tempfile
import time

import hashi_codec as scale
import burn_release
import incoming_release
import incoming_replay_release
import execution_release
import history_release

FORMAT = "sora-nexus-recovery-release-candidate-v1"
MANIFEST = "sora-nexus-recovery-release-v1"
CHECKPOINT_ROOT = "0x79bda41a1db53470463653f358890f2d1fca8ad622c33a1d7095294b0b140452"
RUNTIME_ROLES = {"bootstrapWasm": (134, 131), "holdWasm": (135, 132), "finalWasm": (138, 134)}
REQUIRED = set(RUNTIME_ROLES) | {"nodeBinary", "chainSpec", "policyScale", "operatorsScale", "bootstrapBinding", "holdBinding",
    "bootstrapProvenance", "holdProvenance", "finalProvenance", "signedMappingCsv", "ownershipProofs", "derivedAccountInventory", "semanticPlan",
    "checkpointCaptureManifest", "operatorSigningStop", "externalFinalizedHeads", "incomingReconciliation", "outgoingReconciliation",
    "attackerRegistry", "attackerCoverageEvidence", "bridgeSigningReview", "nativeHashiReview", "burnEntitlements", "burnHistoryManifest", "stakingRoleReview", "exchangeNotice", "operatorInstructions"}
REQUIRED.update(burn_release.ROLES)
REQUIRED.update(incoming_release.ROLES)
GATES = (
    "checkpointData", "checkpointFinalityAndHistory", "executingArtifactIdentity", "accountOwnership", "derivedAccountControl",
    "completeStorageSemantics", "attackerAssetsDebtAndIssuance", "bridgeReconciliation", "incomingPendingCoverage", "burnEntitlementsAndClaimHolds",
    "operatorSigningStopAndFreshKeys", "retainedHistoryStateRootReplay", "fullAndWarpSynchronization",
    "chunksCrashesAndRestarts", "automaticActivationEscapeAndForwardRepair", "maintenanceEpochLivenessAndStaking",
    "bridgeHaltNonceAndAcknowledgements", "fullClippyRuntimeAndProductionRehearsals",
)
MAX_FILE = 2 * 1024**3
MAX_WASM = 50 * 1024**2
MAX_POLICY = 2 * 1024**2
LEGACY_TREE = "36c466398e3f2760e8ab44fdb89de796f5b0dd99"


def reject(condition, message):
    scale.require(condition, message)


def digest_file(path):
    sha, blake, size = hashlib.sha256(), hashlib.blake2b(digest_size=32), 0
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            size += len(block)
            reject(size <= MAX_FILE, "Artifact exceeds file bound")
            sha.update(block)
            blake.update(block)
    return {"sha256": sha.hexdigest(), "blake2_256": blake.hexdigest(), "bytes": size}


def local_file(root, relative):
    reject(isinstance(relative, str) and relative and "\\" not in relative, "Invalid artifact path")
    parsed = PurePosixPath(relative)
    reject(not parsed.is_absolute() and all(part not in ("", ".", "..") for part in parsed.parts), "Artifact path must stay under candidate directory")
    reject(parsed.as_posix() == relative, "Artifact path is not canonical")
    reject(not any("sydneycreds" in part.lower() or part.lower() in (".env", "keystore") for part in parsed.parts), "Credential material is not a public release input")
    current = root
    for part in parsed.parts:
        current = current / part
        reject(not current.is_symlink(), "Symlink inputs are forbidden")
    reject(current.is_file() and stat.S_ISREG(current.stat().st_mode), "Artifact must be a regular file")
    return current


def hex_bytes(value, size=32):
    return scale.raw(value, size, True)


def bounded_hex(value, maximum):
    reject(isinstance(value, str) and len(value) <= maximum * 2 + 2 and re.fullmatch(r"0x(?:[0-9a-f]{2})*", value), "Invalid or oversized canonical hex")
    return bytes.fromhex(value[2:])


def account(value):
    if value.startswith("0x"):
        return scale.raw(value, 32)
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    reject(1 <= len(value) <= 50 and all(char in alphabet for char in value), "Invalid SS58 alphabet/length")
    number = 0
    for char in value:
        number = number * 58 + alphabet.index(char)
    data = b"\0" * (len(value) - len(value.lstrip("1"))) + number.to_bytes((number.bit_length() + 7) // 8, "big")
    reject(data and data[0] < 128, "Invalid SS58 format")
    prefix = 1 if data[0] < 64 else 2
    reject(len(data) == prefix + 34, "Expected SS58 AccountId32")
    identifier = data[0] if prefix == 1 else ((data[0] & 0x3f) << 2) | (data[1] >> 6) | ((data[1] & 0x3f) << 8)
    reject(identifier not in (46, 47) and (prefix == 1 or identifier >= 64), "Reserved/noncanonical SS58 format")
    reject(hashlib.blake2b(b"SS58PRE" + data[:-2], digest_size=64).digest()[:2] == data[-2:], "SS58 checksum mismatch")
    return data[prefix:-2]


def mapping_identity(path):
    reject(path.stat().st_size <= MAX_POLICY, "Mapping CSV exceeds bound")
    lines = path.read_text(encoding="utf-8-sig").splitlines()
    def fields(line):
        parts = [part.strip() for part in line.split(",")]
        result = []
        for part in parts:
            if part.startswith('"'):
                reject(part.endswith('"'), "Unclosed CSV field")
                part = part[1:-1]
            reject(part and '"' not in part, "Invalid CSV field")
            result.append(part)
        return result
    reject(lines and fields(lines[0]) == ["old account address", "new account address"], "Unexpected mapping CSV header")
    pairs = []
    for line in lines[1:]:
        if line.strip():
            row = fields(line)
            reject(len(row) == 2, "Mapping CSV must have two columns")
            pairs.append(tuple(account(value).hex() for value in row))
    reject(pairs and len(pairs) <= 4096, "Missing or oversized signed mapping")
    return sorted(pairs)


def policy_identity(data):
    reject(len(data) <= MAX_POLICY, "Policy SCALE exceeds bound")
    reader = scale.Reader(data)
    reject(int.from_bytes(reader.take(4), "little") == 1, "Unknown policy format")
    reject(reader.take(32) == scale.raw(scale.CHECKPOINT_HASH), "Wrong policy checkpoint")
    reject(reader.take(32) == scale.raw(CHECKPOINT_ROOT), "Wrong policy checkpoint state root")
    count = reader.compact()
    reject(count <= 4096, "Policy mapping bound")
    mappings = [(reader.take(32), reader.take(32)) for _ in range(count)]
    old, new = [row[0] for row in mappings], [row[1] for row in mappings]
    reject(old == sorted(set(old)) and len(set(new)) == len(new) and set(old).isdisjoint(new), "Invalid policy mapping")
    reject(scale.raw(scale.ATTACKER_SORA) not in old + new, "Attacker overlaps replacements")
    count = reader.compact()
    reject(0 < count <= 100, "Missing or oversized recovered council")
    council = [reader.take(32) for _ in range(count)]
    reject(council == sorted(set(council)) and set(council).isdisjoint(old) and scale.raw(scale.ATTACKER_SORA) not in council, "Invalid recovered council")
    chunks = []
    for _ in range(2):
        count = reader.compact()
        reject(0 < count <= 16384, "Missing or oversized work/audit commitment list")
        hashes = [reader.take(32) for _ in range(count)]
        reject(all(any(value) for value in hashes), "Empty work/audit commitment")
        chunks.append(hashes)
    evidence, audit = reader.take(32), reader.take(32)
    reject(any(evidence) and any(audit), "Missing evidence/terminal audit commitment")
    reader.end()
    return {"commitment": hashlib.blake2b(data, digest_size=32).hexdigest(), "evidenceCommitment": evidence.hex(),
            "mapping": [(old.hex(), new.hex()) for old, new in mappings], "replacements": len(mappings), "council": len(council), "workChunks": len(chunks[0]), "auditChunks": len(chunks[1])}


def operator_identity(data):
    reader = scale.Reader(data)
    count = reader.compact()
    reject(0 < count <= 1000, "Missing or oversized operator inventory")
    owners, retired, fresh, babe, online, transitions = [], [], [], [], [], []
    for _ in range(count):
        owners.append(reader.take(32))
        length = reader.compact()
        reject(0 < length <= 3, "Missing or oversized retired-key union")
        keys = [reader.take(32) for _ in range(length)]
        reject(keys == sorted(set(keys)), "Retired operator keys are not canonical")
        retired.extend(keys)
        fresh.append(reader.take(32))
        transitions.append({"owner": owners[-1].hex(), "retired": [key.hex() for key in keys],
                            "fresh": fresh[-1].hex()})
        for label, target in (("BABE", babe), ("im-online", online)):
            length = reader.compact()
            reject(0 < length <= 3, f"Missing or oversized approved {label} key union")
            keys = [reader.take(32) for _ in range(length)]
            reject(keys == sorted(set(keys)) and all(any(key) for key in keys), f"Approved {label} keys are not canonical")
            target.extend(keys)
    reader.end()
    reject(len(set(babe)) == len(babe) and len(set(online)) == len(online), "Retained signing key assigned to multiple owners")
    reject(len(set(owners)) == count and len(set(fresh)) == count and len(set(retired)) == len(retired), "Duplicate owner/key inventory")
    reject(set(retired).isdisjoint(fresh) and all(any(key) for key in retired + fresh), "Reused or empty GRANDPA keys")
    return {"owners": count, "retired": sorted(value.hex() for value in retired), "fresh": sorted(value.hex() for value in fresh),
            "approvedBabe": sorted(value.hex() for value in babe), "approvedImOnline": sorted(value.hex() for value in online),
            "grandpaTransitions": transitions}


def wasm_identity(blob):
    if blob.startswith(bytes([82, 188, 83, 118, 70, 219, 142, 5])):
        from compression import zstd
        with zstd.open(io.BytesIO(blob[8:]), "rb") as source:
            blob = source.read(MAX_WASM + 1)
    reject(len(blob) <= MAX_WASM and blob.startswith(b"\0asm\x01\0\0\0"), "Invalid or oversized Wasm module")
    reader = scale.Reader(blob[8:])
    sections = {}
    def leb(source):
        value, shift = 0, 0
        while shift <= 28:
            byte = source.take(1)[0]
            value |= (byte & 127) << shift
            if not byte & 128:
                reject(value < 2**32, "Oversized Wasm length")
                return value
            shift += 7
        raise ValueError("Invalid Wasm length")
    while reader.offset < len(reader.data):
        kind = reader.take(1)[0]
        body = scale.Reader(reader.take(leb(reader)))
        if kind == 0:
            name = body.take(leb(body)).decode("utf8")
            if name in ("runtime_version", "runtime_apis"):
                reject(name not in sections, "Duplicate runtime identity custom section")
                sections[name] = body.take(len(body.data) - body.offset)
    reject("runtime_version" in sections and "runtime_apis" in sections, "Missing embedded runtime identity")
    reader = scale.Reader(sections["runtime_version"])
    spec_name, impl_name = reader.vector(256).decode(), reader.vector(256).decode()
    authoring, spec, implementation = [int.from_bytes(reader.take(4), "little") for _ in range(3)]
    api_count = reader.compact()
    reject(api_count == 0 and len(sections["runtime_apis"]) % 12 == 0, "Unsupported embedded API encoding")
    transaction = int.from_bytes(reader.take(4), "little")
    system = reader.take(1)[0]
    reader.end()
    reject(spec_name == "sora-substrate", "Wrong runtime spec name")
    return {"specName": spec_name, "implName": impl_name, "authoringVersion": authoring, "specVersion": spec,
            "implVersion": implementation, "transactionVersion": transaction, "systemVersion": system,
            "identitySource": "embedded custom sections; actual Core_version execution remains a release gate"}


def bind_burn_history(root, records, paths):
    """Bind the raw event-proof dependency graph, without treating it as verified."""
    if "burnHistoryManifest" not in paths:
        return
    source = paths["burnHistoryManifest"]
    manifest = scale.read_json(source)
    if manifest.get("schema") == "sora-nexus-burn-evidence-archive-v1":
        from archive_burn_evidence import verify
        references = verify(source)
        reject(len(records) + len(references) <= 16384, "Burn archive artifact inventory bound")
        indexed = set(paths.values())
        for index, (relative, actual) in enumerate(references):
            item = local_file(source.parent, relative)
            if item in indexed:
                continue
            role = f"burnEvidenceShard-{index:05d}"
            reject(role not in records, "Reserved burn archive role collision")
            records[role] = {"role": role, **actual, "object": "objects/" + actual["sha256"], "sourcePath": item.relative_to(root).as_posix(), "executable": False}
            paths[role] = item
        return
    reject(manifest.get("schema") == "sora-nexus-burn-proof-stream-v1" and manifest.get("checkpointHash") == scale.CHECKPOINT_HASH,
           "Wrong raw burn-history manifest identity")
    reject(type(manifest.get("first")) is int and type(manifest.get("last")) is int and
           0 < manifest["first"] <= manifest["last"] <= scale.CHECKPOINT, "Invalid raw burn-history range")
    reject(isinstance(manifest.get("pages"), list) and manifest["pages"] and len(manifest["pages"]) <= 16384,
           "Burn-history page inventory bound")
    indexed = {path: role for role, path in paths.items()}
    serial = 0

    def add(relative, expected):
        nonlocal serial
        reject(isinstance(expected, str) and re.fullmatch(r"[0-9a-f]{64}", expected), "Invalid burn evidence digest")
        item = local_file(source.parent, relative)
        actual = digest_file(item)
        reject(actual["sha256"] == expected, "Burn evidence artifact hash mismatch")
        if item not in indexed:
            reject(len(records) < 16384, "Burn evidence artifact inventory bound; shard larger histories before release")
            role = f"burnHistoryFile-{serial:05d}"
            serial += 1
            reject(role not in records, "Reserved burn evidence role collision")
            records[role] = {"role": role, **actual, "object": "objects/" + actual["sha256"],
                             "sourcePath": item.relative_to(root).as_posix(), "executable": False}
            paths[role] = item
            indexed[item] = role
        return item

    def proof(record):
        refs = record.get("proof")
        reject(isinstance(refs, list) and 0 < len(refs) <= 16384, "Invalid burn proof object list")
        reject(len(set(refs)) == len(refs), "Duplicate burn proof object")
        for digest in refs:
            reject(isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest), "Invalid burn proof object digest")
            add("objects/" + digest, digest)

    initial = manifest.get("initial", {})
    initial_path = add(initial.get("file"), initial.get("sha256"))
    reject(initial_path.stat().st_size <= 64 * 1024**2, "Initial burn evidence bound")
    proof(scale.read_json(initial_path))
    seen = {initial["file"]}
    for page in manifest["pages"]:
        reject(isinstance(page, dict) and set(page) == {"file", "sha256"} and page["file"] not in seen, "Duplicate/invalid burn page binding")
        seen.add(page["file"])
        item = add(page["file"], page["sha256"])
        with gzip.open(item, "rb") as compressed:
            decoded = compressed.read(64 * 1024**2 + 1)
        reject(len(decoded) <= 64 * 1024**2, "Uncompressed burn page bound")
        parsed = json.loads(decoded)
        reject(parsed.get("schema") == "sora-nexus-burn-proof-page-v1" and isinstance(parsed.get("records"), list)
               and 0 < len(parsed["records"]) <= 16, "Invalid raw burn page")
        for record in parsed["records"]:
            proof(record)


def verify_candidate(path, previous_manifest=None):
    root = path.parent.resolve()
    candidate = scale.read_json(path)
    reject(set(candidate) == {"format", "checkpoint", "nodeVersion", "artifacts", "evidence"}, "Unknown candidate fields; readiness booleans are not inputs")
    reject(candidate["format"] == FORMAT and candidate["nodeVersion"] == "4.8.13", "Wrong release identity")
    reject(candidate["checkpoint"] == {"number": scale.CHECKPOINT, "hash": scale.CHECKPOINT_HASH}, "Wrong checkpoint identity")
    reject(isinstance(candidate["artifacts"], list) and 0 < len(candidate["artifacts"]) <= 16384, "Artifact inventory bound")
    records, paths = {}, {}
    for record in candidate["artifacts"]:
        reject(set(record) == {"role", "path", "sha256", "blake2_256"}, "Unexpected artifact fields")
        role = record["role"]
        reject(isinstance(role, str) and re.fullmatch(r"[A-Za-z][A-Za-z0-9_.-]{0,127}", role) and role not in records, "Duplicate or invalid artifact role")
        reject(all(re.fullmatch(r"[0-9a-f]{64}", record[name]) for name in ("sha256", "blake2_256")), "Invalid artifact digest")
        source = local_file(root, record["path"])
        actual = digest_file(source)
        reject(all(actual[name] == record[name] for name in ("sha256", "blake2_256")), f"Artifact digest mismatch: {role}")
        records[role] = {"role": role, **actual, "object": "objects/" + actual["sha256"], "sourcePath": record["path"], "executable": bool(source.stat().st_mode & 0o111)}
        paths[role] = source
    # Bind every manifest-referenced capture file, preserving paths for offline
    # reconstruction. A manifest alone is not a reproducible checkpoint bundle.
    if "checkpointCaptureManifest" in paths:
        source = paths["checkpointCaptureManifest"]
        reject(source.name == "capture-complete.json", "Native capture verifier requires capture-complete.json")
        capture = scale.read_json(source)
        reject(capture.get("schema") == "sora-nexus-checkpoint-capture-v1" and capture.get("checkpoint", {}).get("hash") == scale.CHECKPOINT_HASH, "Wrong checkpoint capture identity")
        references = dict(capture.get("artifacts", {}))
        for trie in [capture.get("top", {}), *capture.get("children", [])]:
            for page in trie.get("pages", []):
                reject(page["file"] not in references, "Duplicate capture file binding")
                references[page["file"]] = {"sha256": page["sha256"]}
        reject(0 < len(references) + len(records) <= 16384, "Capture artifact inventory bound")
        indexed = {path: role for role, path in paths.items()}
        for index, (relative, binding) in enumerate(sorted(references.items())):
            item = local_file(source.parent, relative)
            actual = digest_file(item)
            reject(actual["sha256"] == binding.get("sha256") and ("bytes" not in binding or actual["bytes"] == binding["bytes"]), "Checkpoint capture artifact hash mismatch")
            if item in indexed:
                continue
            role = f"checkpointFile-{index:05d}"
            reject(role not in records, "Reserved checkpoint artifact role collision")
            records[role] = {"role": role, **actual, "object": "objects/" + actual["sha256"], "sourcePath": item.relative_to(root).as_posix(), "executable": bool(item.stat().st_mode & 0o111)}
            paths[role] = item
    bind_burn_history(root, records, paths)
    missing = sorted(REQUIRED - set(records))
    identities = {}
    for role, expected in RUNTIME_ROLES.items():
        if role in paths:
            reject(records[role]["bytes"] <= MAX_WASM, "Wasm artifact exceeds bound")
            identity = wasm_identity(paths[role].read_bytes())
            reject((identity["specVersion"], identity["transactionVersion"]) == expected, f"Wrong {role} version")
            identities[role] = {**identity, "codeHash": "0x" + records[role]["blake2_256"]}
    reject(len({records[role]["blake2_256"] for role in RUNTIME_ROLES if role in records}) == len(identities), "Recovery runtime artifacts must be distinct")
    policy = policy_identity(paths["policyScale"].read_bytes()) if "policyScale" in paths else None
    operators = operator_identity(paths["operatorsScale"].read_bytes()) if "operatorsScale" in paths else None
    incoming_replay = incoming_replay_release.bind(root, records, paths, policy, local_file, digest_file, policy_identity, wasm_identity)
    incoming_release.bind_work(root, records, paths, incoming_replay["activePolicy"] if incoming_replay else policy, local_file, digest_file)
    if "stakingRoleReview" in paths:
        review = scale.read_json(paths["stakingRoleReview"])
        reject(set(review) in ({"evidenceHash", "operatorsHash", "records"},
                              {"evidenceHash", "operatorsHash", "records", "retainedNominations"}),
               "Unknown staking role review fields")
        hex_bytes(review["evidenceHash"])
        hex_bytes(review["operatorsHash"])
        reject("operatorsScale" in records and review["operatorsHash"] == "0x" + records["operatorsScale"]["blake2_256"],
               "Staking roles differ from bootstrap operator inventory")
        # This is only a binding check. Public operator records, nomination
        # signatures and third-party grants still need the native compiler and
        # independently approved operator evidence; a JSON flag cannot approve them.
        reject(isinstance(review["records"], list) and 0 < len(review["records"]) <= 100_000,
               "Missing or oversized staking role review")
    if policy and "signedMappingCsv" in paths:
        reject(mapping_identity(paths["signedMappingCsv"]) == policy["mapping"], "Signed CSV differs from compiled policy mapping")
    if "semanticPlan" in paths and "derivedAccountInventory" in paths:
        semantic = scale.read_json(paths["semanticPlan"])
        derived = scale.read_json(paths["derivedAccountInventory"])
        reject(isinstance(semantic, dict) and isinstance(derived, list) and len(derived) <= 4096 and
               json.dumps(semantic.get("derivedAccounts", []), sort_keys=True) == json.dumps(derived, sort_keys=True),
               "Derived account inventory differs from semantic plan")
    config = None
    if "chainSpec" in paths:
        spec = scale.read_json(paths["chainSpec"])
        reject(spec.get("id") == "sora-nexus-recovery-28191817" and spec.get("protocolId") == "sora-nexus-recovery-28191817-v1" and spec.get("forkId") == "recovery-28191817-v1", "Wrong recovery network identity")
        config = spec.get("recovery")
        reject(isinstance(config, dict) and config.get("schemaVersion") == 1 and config.get("checkpointHash") == scale.CHECKPOINT_HASH, "Missing recovery chain extension")
        for role, field in (("bootstrapWasm", "bootstrapCodeHash"), ("holdWasm", "holdCodeHash"), ("finalWasm", "finalCodeHash")):
            if role in records:
                reject(config.get(field) == "0x" + records[role]["blake2_256"], f"Chain spec {role} mismatch")
        if policy:
            reject(config.get("policyCommitment") == "0x" + policy["commitment"] and config.get("evidenceCommitment") == "0x" + policy["evidenceCommitment"], "Chain policy/evidence mismatch")
        if operators:
            reject(sorted(value[2:] for value in config.get("retiredGrandpaKeys", [])) == operators["retired"] and sorted(value[2:] for value in config.get("approvedGrandpaKeys", [])) == operators["fresh"], "Chain operator-key inventory mismatch")
        reject(type(config.get("firstRecoverySlot")) is int and type(config.get("lastAbandonedSignedSlot")) is int and 0 < config["lastAbandonedSignedSlot"] < config["firstRecoverySlot"] < 2**64, "Invalid signing-stop slot boundary")
        reject(type(config.get("previousGrandpaSetId")) is int and 0 <= config["previousGrandpaSetId"] < 2**64 - 1, "Invalid previous GRANDPA set")
        required_config = {"schemaVersion", "checkpointHash", "policyCommitment", "bootstrapCodeHash", "finalCodeHash", "holdCodeHash", "evidenceCommitment", "previousGrandpaSetId", "previousGrandpaAuthorities", "grandpaAuthorities", "retiredGrandpaKeys", "approvedGrandpaKeys", "lastAbandonedSignedSlot", "firstRecoverySlot"}
        reject(set(config) == required_config, "Unknown recovery chain extension field")
        inventories = []
        for field, maximum in (("retiredGrandpaKeys", 3000), ("approvedGrandpaKeys", 1000)):
            keys = [hex_bytes(value) for value in config[field]]
            reject(0 < len(keys) <= maximum and keys == sorted(set(keys)), "Invalid complete GRANDPA inventory")
            inventories.append(set(keys))
        old_keys, new_keys = inventories
        reject(old_keys.isdisjoint(new_keys), "Fresh GRANDPA inventory reuses retired keys")
        prior, fresh = config["previousGrandpaAuthorities"], config["grandpaAuthorities"]
        reject(0 < len(prior) == len(fresh) <= 1000, "Missing/mismatched active authority inventory")
        prior_seen, fresh_seen = set(), set()
        transitions = {old: row["fresh"] for row in operators["grandpaTransitions"] for old in row["retired"]} if operators else None
        for before, after in zip(prior, fresh):
            reject(isinstance(before, list) and isinstance(after, list) and len(before) == len(after) == 2, "Invalid authority/weight encoding")
            old_key, new_key = hex_bytes(before[0]), hex_bytes(after[0])
            reject(type(before[1]) is int and type(after[1]) is int and 0 < before[1] == after[1] < 2**64, "GRANDPA weights/order changed")
            reject(old_key in old_keys and new_key in new_keys and old_key not in prior_seen and new_key not in fresh_seen, "Incomplete/duplicate active GRANDPA inventory")
            # Runtime installation derives each fresh authority from its old
            # owner. Aggregate key-set equality cannot bind that relationship:
            # swapping two approved keys can attach the wrong voting weight, or
            # select a fresh key belonging only to an inactive approved owner.
            if transitions is not None:
                reject(transitions.get(old_key.hex()) == new_key.hex(),
                       "Active GRANDPA transition differs from the compiled operator owner")
            prior_seen.add(old_key)
            fresh_seen.add(new_key)
        versions = set()
        for number, encoded in spec.get("codeSubstitutes", {}).items():
            reject(number == str(int(number)) and 0 <= int(number) < 2**32, "Noncanonical or out-of-range substitute height")
            code = bounded_hex(encoded, MAX_WASM)
            identity = wasm_identity(code)
            reject(identity["specVersion"] not in versions, "Duplicate substitute spec version")
            versions.add(identity["specVersion"])
            if int(number) == scale.CHECKPOINT:
                reject(identity["specVersion"] == 134 and identity["transactionVersion"] == 131, "Wrong checkpoint substitute identity")
                reject("bootstrapWasm" in paths and code == paths["bootstrapWasm"].read_bytes(), "Checkpoint substitute bytes differ from immutable bootstrap")
            else:
                reject(int(number) < scale.CHECKPOINT and identity["specVersion"] < 134,
                       "Unapproved substitute overlaps recovery execution")
        reject(str(scale.CHECKPOINT) in spec.get("codeSubstitutes", {}), "Missing immutable checkpoint substitute")
    for role in ("bootstrapBinding", "holdBinding"):
        if role not in paths:
            continue
        binding = scale.read_json(paths[role])
        required = {"policyScale", "policyCommitment", "operatorsScale", "previousGrandpaSetId", "firstRecoverySlot", "finalWasm"} | ({"holdWasm"} if role == "bootstrapBinding" else set())
        reject(set(binding) == required, "Unexpected build binding fields")
        if policy:
            reject(binding["policyCommitment"] == "0x" + policy["commitment"] and bounded_hex(binding["policyScale"], MAX_POLICY) == paths["policyScale"].read_bytes(), "Build binding policy mismatch")
        if operators:
            reject(bounded_hex(binding["operatorsScale"], MAX_POLICY) == paths["operatorsScale"].read_bytes(), "Build binding operator mismatch")
        if config:
            reject(binding["previousGrandpaSetId"] == config["previousGrandpaSetId"] and binding["firstRecoverySlot"] == config["firstRecoverySlot"], "Build/chain consensus binding mismatch")
        for name in ("finalWasm", "holdWasm"):
            if name in binding and name in paths:
                referenced = local_file(paths[role].parent, binding[name])
                reject(digest_file(referenced) == {key: records[name][key] for key in ("sha256", "blake2_256", "bytes")}, "Embedded destination artifact mismatch")
    for role, binding_role, spec_version in (("bootstrapProvenance", "bootstrapBinding", 134), ("holdProvenance", "holdBinding", 135)):
        if role not in paths:
            continue
        provenance = scale.read_json(paths[role])
        reject(provenance.get("formatVersion") == 1 and provenance.get("deployedSourceTree") == LEGACY_TREE and provenance.get("sdk") == "polkadot-stable2603-3", "Wrong preserved-history source provenance")
        reject(provenance.get("specVersion") == spec_version and provenance.get("transactionVersion") == (131 if spec_version == 134 else 132), "Wrong provenance runtime version")
        reject(provenance.get("bound") is True and isinstance(provenance.get("bindingEvidence"), dict), "Unbound development export cannot be packaged as a release artifact")
        for field, target in (("binding", binding_role), ("policy", "policyScale"), ("operators", "operatorsScale"), ("finalWasm", "finalWasm"), ("holdWasm", "holdWasm")):
            if field == "holdWasm" and spec_version == 135:
                reject(provenance["bindingEvidence"].get(field) is None, "Unexpected hold dependency")
            elif target in records:
                reject(provenance["bindingEvidence"].get(field) == {key: records[target][key] for key in ("sha256", "blake2_256")}, "Provenance binding hash mismatch")
    for role in ("bootstrapProvenance", "holdProvenance", "finalProvenance"):
        if role in paths:
            provenance = scale.read_json(paths[role])
            reject(provenance.get("fixtureOnly") is not True and
                   not str(provenance.get("schema", "")).startswith("sora-nexus-public-fixture") and
                   provenance.get("schema") != "sora-nexus-final-fixture-source-v1",
                   "Public diagnostic fixture artifacts cannot be packaged for production")
    historical_replay = history_release.bind(root, records, paths, local_file, digest_file)
    previous_hash = None
    if previous_manifest:
        prior = scale.read_json(previous_manifest)
        reject(prior.get("format") == MANIFEST and prior.get("checkpoint") == candidate["checkpoint"], "Unknown prior release manifest/checkpoint")
        previous_hash = digest_file(previous_manifest)["sha256"]
        prior_bootstrap = next((record for record in prior.get("artifacts", []) if record["role"] == "bootstrapWasm"), None)
        reject(prior_bootstrap and "bootstrapWasm" in records and prior_bootstrap["blake2_256"] == records["bootstrapWasm"]["blake2_256"], "Immutable spec-134 bootstrap cannot be replaced")
    reject(isinstance(candidate["evidence"], list), "Evidence must name raw artifacts, not pass booleans")
    evidence, seen = [], set()
    for entry in candidate["evidence"]:
        reject(set(entry) == {"gate", "artifacts"} and entry["gate"] in GATES and entry["gate"] not in seen, "Unknown/duplicate evidence gate")
        seen.add(entry["gate"])
        reject(isinstance(entry["artifacts"], list) and entry["artifacts"] and len(set(entry["artifacts"])) == len(entry["artifacts"]), "Missing/duplicate evidence references")
        reject(all(role in records for role in entry["artifacts"]), "Evidence references missing artifact")
        evidence.append(entry)
    report = {"format": MANIFEST, "checkpoint": candidate["checkpoint"], "nodeVersion": "4.8.13", "nodeIdentitySource": "expected release version; executable identity remains unverified", "productionReady": False,
        "artifacts": sorted(records.values(), key=lambda row: row["role"]), "runtimeIdentities": identities,
        "policy": policy, "operators": operators, "previousManifestSha256": previous_hash,
        "candidateSha256": digest_file(path)["sha256"], "missingArtifacts": missing, "evidence": evidence,
        "gates": {gate: {"status": "unverified", "reason": "Raw evidence is bound, but no independent executable verification has cleared this gate"} for gate in GATES},
        "blockers": ["Exact-artifact runtime execution, full/warp synchronization and scenario verifiers are required; source self-reports cannot authorize release"]}
    if incoming_replay is not None:
        report["incomingRecoveryReplay"] = incoming_replay
    if historical_replay is not None:
        report["historicalReplay"] = historical_replay
    report["blockers"] += ["Missing required artifact: " + role for role in missing]
    return report, paths


def execute_verifier(binary, arguments):
    # Keep untrusted stdout/stderr off the Python heap and enforce an output cap
    # while the explicitly pinned native verifier is running.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen([str(binary.resolve()), *arguments], stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
        deadline = time.monotonic() + 3600
        try:
            while process.poll() is None:
                reject(time.monotonic() < deadline, "Native verifier timed out")
                reject(os.fstat(stdout.fileno()).st_size <= 4 * 1024**2 and os.fstat(stderr.fileno()).st_size <= 4 * 1024**2, "Verifier output bound")
                time.sleep(0.05)
            stdout.seek(0)
            stderr.seek(0)
            out, err = stdout.read(4 * 1024**2 + 1), stderr.read(4 * 1024**2 + 1)
            reject(len(out) <= 4 * 1024**2 and len(err) <= 4 * 1024**2, "Verifier output bound")
            return subprocess.CompletedProcess(arguments, process.returncode, out, err)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()


def run_evidence_verifier(report, paths, binary, expected_sha256, replay_scratch=None):
    reject(binary.is_file() and not binary.is_symlink() and digest_file(binary)["sha256"] == expected_sha256, "Explicit local verifier binary hash mismatch")
    checks = []
    if "checkpointCaptureManifest" in paths:
        capture = paths["checkpointCaptureManifest"].parent
        checks += [("checkpointData", ["verify-capture", "--capture", str(capture)]),
                   ("checkpointMetadata", ["verify-metadata", "--capture", str(capture)])]
    ownership_inputs = {"signedMappingCsv", "ownershipProofs", "checkpointCaptureManifest", "semanticPlan", "derivedAccountInventory"}
    if ownership_inputs <= paths.keys():
        checks.append(("accountOwnership", ["verify-ownership", "--csv", str(paths["signedMappingCsv"]),
            "--proofs", str(paths["ownershipProofs"]), "--capture", str(paths["checkpointCaptureManifest"].parent),
            "--semantic", str(paths["semanticPlan"]), "--derived-inventory", str(paths["derivedAccountInventory"])]))
    incoming_arguments = incoming_release.arguments(paths)
    if incoming_arguments:
        if "recoveryParentReplay" in paths:
            reject(replay_scratch is not None and replay_scratch.is_dir() and not any(replay_scratch.iterdir()), "Owned fresh incoming replay scratch required")
            incoming_arguments += ["--recovery-output", str(replay_scratch / "replay")]
        checks.append(("incomingPendingCoverage", incoming_arguments))
    bindings = {record["role"]: record["sha256"] for record in report["artifacts"]}
    receipts = []
    for gate, arguments in checks:
        if gate == "incomingPendingCoverage" and "recoveryParentReplay" in paths:
            before = {role: digest_file(path) for role, path in paths.items()}
            with (replay_scratch / "inputs-before.json").open("x") as stream:
                json.dump(before, stream, sort_keys=True, indent=2)
            try:
                completed = incoming_replay_release.execute(binary, arguments, replay_scratch, expected_sha256)
            finally:
                after = {}
                for role, path in paths.items():
                    try:
                        after[role] = digest_file(path)
                    except (OSError, ValueError) as error:
                        after[role] = {"error": str(error)}
                with (replay_scratch / "input-binding.json").open("x") as stream:
                    json.dump({"before":before,"after":after,"inputChanges":[] if before==after else ["input"],"productionReady":False},stream,sort_keys=True,indent=2)
            reject(before == after, "Incoming replay changed an input artifact")
        else:
            completed = execute_verifier(binary, arguments)
        reject(len(completed.stdout) <= 4 * 1024**2 and len(completed.stderr) <= 4 * 1024**2, "Verifier output bound")
        receipt = {"gate": gate, "verifierSha256": expected_sha256, "arguments": arguments, "exitCode": completed.returncode,
                   "stdoutSha256": hashlib.sha256(completed.stdout).hexdigest(), "stderrSha256": hashlib.sha256(completed.stderr).hexdigest()}
        result = (incoming_replay_release.parse_json(completed.stdout) if gate == "incomingPendingCoverage" and "recoveryParentReplay" in paths else json.loads(completed.stdout)) if completed.returncode == 0 else None
        if gate == "checkpointData":
            valid = result and result.get("schema") == "sora-nexus-checkpoint-verification-v1" and result.get("checkpointHash") == scale.CHECKPOINT_HASH and result.get("stateRoot") == CHECKPOINT_ROOT and result.get("captureManifestSha256") == bindings["checkpointCaptureManifest"] and all(result.get(field) is True for field in ("headerHashVerified", "extrinsicsRootVerified", "stateRootVerified"))
        elif gate == "incomingPendingCoverage":
            valid = incoming_release.valid_receipt(result, report, bindings, CHECKPOINT_ROOT)
            if "recoveryParentReplay" in paths:
                incoming_replay_release.preserve_outputs(report, paths, replay_scratch, digest_file)
        elif gate == "accountOwnership":
            controls = result.get("controls", {}) if isinstance(result, dict) else {}
            metadata = result.get("checkpointMetadata", {}) if isinstance(result, dict) else {}
            controls = controls if isinstance(controls, dict) else {}
            metadata = metadata if isinstance(metadata, dict) else {}
            direct, derived = controls.get("directAccounts"), controls.get("derivedAccounts")
            expected_owners = ["0x" + old for old, _ in report["policy"]["mapping"]] if report["policy"] else None
            valid = bool(isinstance(result, dict) and report["policy"] and
                result.get("schema") == "sora-nexus-account-control-verification-v1" and
                result.get("ownershipVerified") is True and result.get("derivedControlVerified") is True and
                type(result.get("accounts")) is int and result["accounts"] == report["policy"]["replacements"] and
                result.get("checkpointHash") == scale.CHECKPOINT_HASH and result.get("checkpointStateRoot") == CHECKPOINT_ROOT and
                all(result.get(field) == bindings[role] for field, role in (
                    ("mappingCsvSha256", "signedMappingCsv"), ("ownershipProofsSha256", "ownershipProofs"),
                    ("semanticPlanSha256", "semanticPlan"), ("derivedInventorySha256", "derivedAccountInventory"),
                    ("captureManifestSha256", "checkpointCaptureManifest"))) and
                metadata.get("schema") == "sora-nexus-checkpoint-metadata-witness-v1" and
                metadata.get("checkpointHash") == scale.CHECKPOINT_HASH and metadata.get("stateRoot") == CHECKPOINT_ROOT and
                metadata.get("wasmExecutionVerified") is True and metadata.get("nativeExecution") is False and
                controls.get("schema") == "sora-nexus-semantic-ownership-verification-v1" and
                controls.get("checkpointHash") == scale.CHECKPOINT_HASH and
                type(controls.get("mappedAccounts")) is int and controls["mappedAccounts"] == result["accounts"] and
                controls.get("replacementOwnershipVerified") is True and controls.get("derivedControlGraphVerified") is True and
                isinstance(direct, list) and isinstance(derived, list) and
                all(isinstance(owner, str) for owner in direct + derived) and sorted(direct + derived) == expected_owners)
        else:
            valid = result and result.get("schema") == "sora-nexus-checkpoint-metadata-witness-v1" and result.get("checkpointHash") == scale.CHECKPOINT_HASH and result.get("stateRoot") == CHECKPOINT_ROOT and result.get("wasmExecutionVerified") is True and result.get("nativeExecution") is False
        receipt["result"] = result
        receipts.append(receipt)
        if gate in report["gates"]:
            report["gates"][gate] = {"status": "verified" if valid else "failed", "reason": "Fresh execution of explicitly pinned local native verifier", "receipt": receipt}
        if gate == "accountOwnership":
            report["gates"]["derivedAccountControl"] = {"status": "verified" if valid else "failed",
                "reason": "Exact checkpoint and mapping-bound control graph; complete derived-account discovery remains a separate storage-coverage gate",
                "receipt": receipt}
    # Metadata verification here executes checkpoint code only, not recovery artifacts.
    report["nativeVerifierReceipts"] = receipts
    report["productionReady"] = False


def package(candidate, output, previous_manifest=None, verifier=None, verifier_sha256=None, *,
            burn_node=None, burn_node_sha256=None, burn_verifier_sha256=None,
            burn_verification_output=None, burn_timeout=86400,
            execution_verifier_sha256=None, execution_verification_output=None):
    reject(not output.exists(), "Release output must be a new directory")
    report, paths = verify_candidate(candidate, previous_manifest)
    if verifier:
        reject(isinstance(verifier_sha256, str) and re.fullmatch(r"[0-9a-f]{64}", verifier_sha256), "Pin the explicitly supplied verifier SHA-256")
        replay_scratch = None
        if "recoveryParentReplay" in paths:
            output.parent.mkdir(parents=True, exist_ok=True)
            replay_scratch = Path(tempfile.mkdtemp(prefix=".sora-nexus-incoming-replay-", dir=output.parent)).resolve()
        if replay_scratch is None:
            run_evidence_verifier(report, paths, verifier, verifier_sha256)
        else:
            run_evidence_verifier(report, paths, verifier, verifier_sha256, replay_scratch)
        history_release.run(report, paths, verifier, verifier_sha256, output.parent, local_file, digest_file)
    else:
        reject(verifier_sha256 is None, "Verifier digest without executable")
    burn_options = (burn_node, burn_node_sha256, burn_verifier_sha256, burn_verification_output)
    if any(value is not None for value in burn_options):
        reject(all(value is not None for value in burn_options) and verifier is not None,
               "Burn verification requires all explicit executable pins, a fresh output directory and the native verifier")
        burn_release.verify(report, paths, native=verifier, native_sha256=verifier_sha256,
                            node=burn_node, node_sha256=burn_node_sha256,
                            bundle_sha256=burn_verifier_sha256, output=burn_verification_output,
                            timeout=burn_timeout)
    if execution_verifier_sha256 is not None or execution_verification_output is not None:
        reject(execution_verifier_sha256 is not None and execution_verification_output is not None,
               "Execution verification requires the fixed verifier source pin and a fresh output directory")
        execution_release.package_hook(report, paths, execution_verifier_sha256, execution_verification_output)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=".sora-nexus-release-", dir=output.parent))
    try:
        (temporary / "objects").mkdir()
        for record in report["artifacts"]:
            target = temporary / record["object"]
            if target.exists():
                continue
            shutil.copyfile(paths[record["role"]], target)
            reject(digest_file(target) == {key: record[key] for key in ("sha256", "blake2_256", "bytes")}, "Artifact changed while packaging")
            executable = any(row["sha256"] == record["sha256"] and row["executable"] for row in report["artifacts"])
            target.chmod(0o555 if executable else 0o444)
        manifest = json.dumps(report, sort_keys=True, indent=2).encode() + b"\n"
        (temporary / "release-manifest.json").write_bytes(manifest)
        checksums = [(hashlib.sha256(manifest).hexdigest(), "release-manifest.json")]
        checksums += [(row["sha256"], row["object"]) for row in report["artifacts"]]
        (temporary / "SHA256SUMS").write_text("".join(f"{digest}  {name}\n" for digest, name in sorted(set(checksums))))
        (temporary / "README.txt").write_text("SORA Nexus recovery release candidate. NOT READY FOR PRODUCTION.\nRead release-manifest.json blockers and gates. Artifacts are content-addressed.\nNo publication, service changes or restart were performed.\n")
        reject(not output.exists(), "Release destination appeared during verification")
        # Reserve the destination with exclusive mkdir. Never rename over a concurrent
        # empty directory. READY is written last; failed moves remain incomplete.
        output.mkdir()
        for child in temporary.iterdir():
            os.rename(child, output / child.name)
        temporary.rmdir()
        (output / "PACKAGE-COMPLETE").write_text(hashlib.sha256(manifest).hexdigest() + "\n")
    except BaseException:
        shutil.rmtree(temporary, ignore_errors=True)
        raise
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--previous-manifest", type=Path)
    parser.add_argument("--evidence-verifier", type=Path)
    parser.add_argument("--evidence-verifier-sha256")
    parser.add_argument("--burn-node", type=Path)
    parser.add_argument("--burn-node-sha256")
    parser.add_argument("--burn-verifier-sha256")
    parser.add_argument("--burn-verification-output", type=Path)
    parser.add_argument("--burn-timeout", type=int, default=86400)
    parser.add_argument("--execution-verifier-sha256")
    parser.add_argument("--execution-verification-output", type=Path)
    args = parser.parse_args()
    result = package(args.candidate, args.output, args.previous_manifest, args.evidence_verifier, args.evidence_verifier_sha256,
                     burn_node=args.burn_node, burn_node_sha256=args.burn_node_sha256,
                     burn_verifier_sha256=args.burn_verifier_sha256,
                     burn_verification_output=args.burn_verification_output, burn_timeout=args.burn_timeout,
                     execution_verifier_sha256=args.execution_verifier_sha256,
                     execution_verification_output=args.execution_verification_output)
    print(json.dumps({"output": str(args.output), "productionReady": result["productionReady"], "missingArtifacts": result["missingArtifacts"], "blockers": result["blockers"]}))


if __name__ == "__main__":
    main()
