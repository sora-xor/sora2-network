#!/usr/bin/env python3
"""Offline, byte-preserving replacement of the three embedded test runtimes.

Nothing is written until all runtime, metadata, source and JSON checks pass.
Use --dry-run to perform those checks without replacing files or writing evidence.
"""

import argparse
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
from datetime import datetime, timezone


PROFILES = {
    "stage": {
        "features": ["build-wasm-binary", "private-net", "stage"],
        "specs": ["chain_spec_bridge_staging.json", "chain_spec_staging.json"],
    },
    "test": {
        "features": ["build-wasm-binary", "private-net", "stage", "wip", "reduced-pswap-reward-periods"],
        "specs": ["chain_spec_test.json"],
    },
}
CODE_KEY = "0x3a636f6465"
CODE_VALUE = re.compile(rb'"0x3a636f6465"\s*:\s*"(0x[0-9a-fA-F]+)"')
CAPACITY_STORAGE = {"PendingOperationsByProposer", "ProposerCountedOperations", "CancellationApprovals"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(value):
    return hashlib.sha256(value).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "Duplicate JSON key: " + key)
        result[key] = value
    return result


def parse_json(value):
    return json.loads(value, object_pairs_hook=unique_pairs)


def run(command, data=None):
    env = os.environ.copy()
    env.pop("NO_COLOR", None)  # subwasm requires a bool, unlike NO_COLOR=1 in this shell.
    result = subprocess.run(command, input=data, capture_output=True, env=env, check=False)
    require(result.returncode == 0, "Command failed: " + " ".join(command) + "\n" + result.stderr.decode(errors="replace")[-3000:])
    return result.stdout


def without_code(document):
    result = copy.deepcopy(document)
    del result["genesis"]["raw"]["top"][CODE_KEY]
    return result


def inspect_wasm(data, cache):
    key = digest(data)
    if key in cache:
        return cache[key]
    # subwasm runs this local byte input; no RPC, network URL or shell is involved.
    metadata = parse_json(run(["subwasm", "metadata", "--format", "json", "/dev/stdin"], data))
    info = parse_json(run(["subwasm", "info", "--json", "/dev/stdin"], data))
    require(set(metadata) == {"V14"}, "Expected V14 metadata")
    require(info["metadata_version"] == 14, "Expected metadata version 14")
    require(info["reserved_meta_valid"], "Invalid metadata prefix")
    model = metadata["V14"]
    pallets = {p["name"]: p for p in model["pallets"]}
    require(len(pallets) == len(model["pallets"]), "Duplicate pallet names")
    indices = {name: pallet["index"] for name, pallet in pallets.items()}
    require(len(set(indices.values())) == len(indices), "Duplicate pallet indices")
    result = {
        "model": model,
        "metadataSha256": digest(canonical(metadata)),
        "pallets": pallets,
        "palletIndices": indices,
        "version": info["core_version"],
        "compression": info["compression"],
    }
    cache[key] = result
    return result


def required_runtime_checks(runtime):
    version = runtime["version"]
    require(version["specVersion"] == 134 and version["transactionVersion"] == 131,
            "Candidate must have spec version 134 and transaction version 131")
    require(version["specName"] == "sora-substrate", "Unexpected runtime identity")
    pallets = runtime["pallets"]
    require("Sudo" in pallets, "Private-network candidate must contain Sudo")
    types = {item["id"]: item["type"] for item in runtime["model"]["types"]["types"]}
    calls = types[pallets["EthBridge"]["calls"]["ty"]]["def"]["variant"]["variants"]
    require(any(c["name"] == "cancel_pending_multisig" and c["index"] == 18 for c in calls),
            "EthBridge.cancel_pending_multisig must have call index 18")
    storage = {entry["name"] for entry in pallets["BridgeMultisig"]["storage"]["entries"]}
    require(CAPACITY_STORAGE <= storage, "BridgeMultisig capacity storage is missing")
    return {
        "runtimeVersion134131": True,
        "privateNetworkSudoPresent": True,
        "cancelPendingMultisigCall18": True,
        "capacityStoragePresent": True,
    }


def atomic_write(path, data, mode=None):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, prefix="." + path.name + ".", delete=False) as handle:
        temporary = Path(handle.name)
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    try:
        if mode is not None:
            temporary.chmod(mode)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--stage-wasm", type=Path, required=True)
    parser.add_argument("--test-wasm", type=Path, required=True)
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    for profile in PROFILES:
        parser.add_argument("--" + profile + "-log", type=Path)
        parser.add_argument("--" + profile + "-command")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    root, output = args.root.resolve(), args.output.resolve()
    provenance_bytes = args.provenance.read_bytes()
    provenance = parse_json(provenance_bytes)
    require(provenance["status"] == "passed", "Source provenance has not passed")
    source_tree = provenance["sourceTreeAfterPatch"]
    require(re.fullmatch(r"[0-9a-f]{40}", source_tree), "Invalid source tree hash")
    for name, expected in provenance["files"].items():
        require(digest((root / name).read_bytes()) == expected, "Source input changed: " + name)
    base_commit = run(["git", "-C", str(root), "rev-parse", "HEAD"]).decode().strip()
    comparator_path = output.parent / "compare-metadata.py"
    if not comparator_path.is_file():
        comparator_path = root / "runtime-upgrade-4.8.12/validation/compare-metadata.py"
    module_spec = importlib.util.spec_from_file_location("chain_spec_metadata_comparison", comparator_path)
    comparison_module = importlib.util.module_from_spec(module_spec)
    module_spec.loader.exec_module(comparison_module)
    report = {
        "schemaVersion": 1,
        "status": "passed",
        "checkedAt": datetime.now(timezone.utc).isoformat(),
        "sourceTree": source_tree,
        "sourceBaseCommit": base_commit,
        "provenanceSha256": digest(provenance_bytes),
        "scriptSha256": digest(Path(__file__).read_bytes()),
        "subwasmVersion": run(["subwasm", "--version"]).decode().strip(),
        "comparatorSha256": digest(comparator_path.read_bytes()),
        "metadataHashEncoding": "UTF-8 JSON sorted keys, compact separators, ensure_ascii=false",
        "networkUsed": False,
        "runtimes": [],
    }
    changes, cache, candidates = [], {}, {}
    for profile, config in PROFILES.items():
        blob_path = getattr(args, profile + "_wasm").resolve()
        blob = blob_path.read_bytes()
        runtime = inspect_wasm(blob, cache)
        checks = required_runtime_checks(runtime)
        metadata_self_checks = comparison_module.self_checks(runtime["model"])
        checks["metadataSelfChecksPassed"] = True
        candidate = {
            "profile": profile,
            "features": config["features"],
            "sourceTree": source_tree,
            "wasm": {
                "path": str(blob_path),
                "sha256": digest(blob),
                "bytes": len(blob),
                "specVersion": runtime["version"]["specVersion"],
                "transactionVersion": runtime["version"]["transactionVersion"],
                "metadataSha256": runtime["metadataSha256"],
                "compression": runtime["compression"],
            },
            "checks": checks,
            "metadataSelfChecks": metadata_self_checks,
            "palletIndices": runtime["palletIndices"],
            "specs": [],
        }
        build_log, build_command = getattr(args, profile + "_log"), getattr(args, profile + "_command")
        require(bool(build_log) == bool(build_command), profile + " log and command must be supplied together")
        if build_log:
            log_bytes = build_log.read_bytes()
            require(b"Finished `release` profile" in log_bytes, profile + " build log has no release completion")
            candidate["build"] = {"command": build_command, "logPath": str(build_log.resolve()),
                                  "logSha256": digest(log_bytes), "logBytes": len(log_bytes),
                                  "sourceTree": source_tree}
        for filename in config["specs"]:
            path = root / "node/chain_spec/src/bytes" / filename
            original = path.read_bytes()
            document = parse_json(original)
            matches = list(CODE_VALUE.finditer(original))
            require(len(matches) == 1, "Expected one raw :code string in " + filename)
            match = matches[0]
            old_hex = document["genesis"]["raw"]["top"][CODE_KEY]
            require(match.group(1).decode() == old_hex, "Code match is not genesis.raw.top :code")
            old_blob = bytes.fromhex(old_hex[2:])
            previous = inspect_wasm(old_blob, cache)
            require(previous["palletIndices"] == runtime["palletIndices"],
                    "Feature pallet indices differ in " + filename)
            comparison = comparison_module.Comparison(previous["model"], runtime["model"]).run()
            require(comparison["compatibleExistingScaleEncoding"],
                    "Incompatible metadata in " + filename + ": " + json.dumps(comparison["breakingChanges"]))
            require(not comparison["constantValueChanges"],
                    "Existing metadata constants changed in " + filename + ": " +
                    json.dumps([item["path"] for item in comparison["constantValueChanges"]]))
            replacement = ("0x" + blob.hex()).encode()
            updated = original[:match.start(1)] + replacement + original[match.end(1):]
            updated_document = parse_json(updated)
            non_code = without_code(document)
            require(without_code(updated_document) == non_code, "Non-code JSON changed in " + filename)
            require(updated_document["genesis"]["raw"]["top"][CODE_KEY] == replacement.decode(),
                    "Runtime replacement mismatch")
            old_masked = original[:match.start(1)] + b"<runtime>" + original[match.end(1):]
            new_match = CODE_VALUE.search(updated)
            new_masked = updated[:new_match.start(1)] + b"<runtime>" + updated[new_match.end(1):]
            require(old_masked == new_masked, "Non-code bytes changed in " + filename)
            candidate["specs"].append({
                "path": str(path.relative_to(root)), "sourceBaseCommit": base_commit,
                "sha256": digest(updated), "bytes": len(updated),
                "previousSha256": digest(original), "previousCodeSha256": digest(old_blob),
                "codeSha256": digest(blob), "codeBytes": len(blob),
                "previousMetadataSha256": previous["metadataSha256"],
                "currentMetadataSha256": runtime["metadataSha256"],
                "previousRuntimeVersion": {name: previous["version"][name] for name in
                                           ("specName", "implName", "specVersion", "transactionVersion")},
                "nonCodeJsonSha256": digest(canonical(non_code)),
                "nonCodeBytesSha256": digest(old_masked),
                "nonCodeValuesUnchanged": True, "nonCodeBytesUnchanged": True,
                "lastRuntimeUpgradeUnchanged": True, "palletIndicesPreserved": True,
                "constantValuesPreserved": True,
                "compatibleExistingScaleEncoding": True,
                "metadataComparison": comparison,
            })
            changes.append((path, original, updated, path.stat().st_mode & 0o777))
        report["runtimes"].append(candidate)
        candidates[profile] = blob
    require(candidates["stage"] != candidates["test"], "Feature profiles must have distinct binaries")
    for path, original, _, _ in changes:
        require(path.read_bytes() == original, "Concurrent spec edit detected: " + str(path))
    if not args.dry_run:
        for path, _, updated, mode in changes:
            atomic_write(path, updated, mode)
            require(path.read_bytes() == updated, "Written spec differs: " + str(path))
        atomic_write(output, json.dumps(report, indent=2).encode() + b"\n")
    print(json.dumps({"status": "validated" if args.dry_run else "passed", "dryRun": args.dry_run,
                      "output": str(output), "sourceTree": source_tree,
                      "profiles": [{"profile": item["profile"], "wasmSha256": item["wasm"]["sha256"],
                                    "specs": [spec["path"] for spec in item["specs"]]}
                                   for item in report["runtimes"]]}, indent=2))


if __name__ == "__main__":
    main()
