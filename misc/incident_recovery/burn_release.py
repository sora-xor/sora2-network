"""Fresh complete Burn-ledger verification, with fixed SHA-pinned executables.

This clears only the derived entitlement/hold gate. Finality, authority approval,
claim-service deployment and all consensus/rehearsal gates remain independent.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time

import hashi_codec as scale

ROLES = {"burnAllocationPolicy", "burnRecoveryVerifier", "burnRecoveryVerifierProvenance",
         "burnNodeBinary", "burnNativeVerifier"}
INPUTS = ROLES | {"burnHistoryManifest", "burnEntitlements", "policyScale", "signedMappingCsv", "attackerCoverageEvidence"}
GATE = "burnEntitlementsAndClaimHolds"
FIRST = 25_043_003
MAX_OUTPUT = 4 * 1024**2
GENERATED_PREFIX = "burnVerified"
GENERATED_PATH = "generated-burn-verification/"


def require(value, reason):
    scale.require(value, reason)


def bind_inputs(report, paths):
    """Compare public allocation ownership to the exact compiled runtime policy."""
    import release_package as package
    require(INPUTS <= paths.keys(), "Missing fixed Burn verification inputs")
    bindings = {row["role"]: row["sha256"] for row in report["artifacts"]}
    for role in INPUTS:
        require(package.digest_file(paths[role])["sha256"] == bindings[role], "Burn input changed after candidate admission: " + role)
    policy = package.policy_identity(paths["policyScale"].read_bytes())
    # Automatic utility descendants are added by the native trusted-mapping
    # verifier. Entitlements below must use the complete expanded policy, while
    # every original CSV pair must still be retained unchanged.
    require(set(package.mapping_identity(paths["signedMappingCsv"])).issubset(policy["mapping"]), "Burn mapping differs from runtime policy")
    allocation = scale.read_json(paths["burnAllocationPolicy"])
    require(isinstance(allocation, dict) and set(allocation) == {"version", "policyHash", "genesisHash", "checkpoint", "substitutions", "attackerAccounts"},
            "Burn allocation policy must exclude post-fork/circular release fields")
    require(allocation["version"] == 1 and type(allocation["version"]) is int and
            allocation["policyHash"] == "0x" + policy["commitment"] and allocation["genesisHash"] == scale.GENESIS_HASH and
            allocation["checkpoint"] == {"block": scale.CHECKPOINT, "hash": scale.CHECKPOINT_HASH}, "Wrong Burn allocation policy identity")
    require(isinstance(allocation["substitutions"], list) and len(allocation["substitutions"]) <= 4096, "Invalid Burn substitutions")
    pairs = []
    for pair in allocation["substitutions"]:
        require(isinstance(pair, dict) and set(pair) == {"old", "new"}, "Invalid Burn substitution")
        pairs.append((package.account(pair["old"]).hex(), package.account(pair["new"]).hex()))
    require(sorted(pairs) == policy["mapping"], "Burn substitutions differ from compiled runtime mapping")
    coverage = scale.read_json(paths["attackerCoverageEvidence"])
    require(isinstance(coverage, dict) and coverage.get("schema") == "sora-nexus-attacker-typed-coverage-v1" and
            isinstance(coverage.get("attackerRegistry"), list), "Burn attackers need the bound native typed registry")
    registry = coverage["attackerRegistry"]
    require(all(isinstance(value, str) and re.fullmatch(r"0x(?:[0-9a-f]{40}|[0-9a-f]{64})", value) for value in registry) and
            len(set(registry)) == len(registry) and scale.ATTACKER_SORA in registry and scale.ATTACKER_EVM in registry,
            "Malformed or incomplete native attacker identities")
    require(isinstance(allocation["attackerAccounts"], list) and len(allocation["attackerAccounts"]) <= 4096, "Invalid Burn attacker list")
    attackers = ["0x" + package.account(value).hex() for value in allocation["attackerAccounts"]]
    require(sorted(attackers) == sorted(value for value in registry if len(value) == 66) and
            not set(attackers).intersection("0x" + account for pair in pairs for account in pair),
            "Burn attacker identities differ from native registry or overlap victims")
    provenance = scale.read_json(paths["burnRecoveryVerifierProvenance"])
    require(isinstance(provenance, dict) and provenance.get("schema") == "sora-nexus-bundled-burn-verifier-v1" and
            provenance.get("artifact", {}).get("sha256") == bindings["burnRecoveryVerifier"] and
            provenance["artifact"].get("bytes") == paths["burnRecoveryVerifier"].stat().st_size and
            isinstance(provenance.get("inputs"), list) and len(provenance["inputs"]) > 0,
            "Bundled Burn verifier provenance mismatch")
    return bindings


def execute(binary, arguments, timeout):
    """No shell, stdin, Node preload flags or caller-defined command strings."""
    environment = {key: value for key, value in os.environ.items() if key not in {
        "NODE_OPTIONS", "NODE_PATH", "LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH"}}
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen([str(binary), *arguments], stdin=subprocess.DEVNULL,
                                   stdout=stdout, stderr=stderr, env=environment, start_new_session=True)
        deadline = time.monotonic() + timeout
        try:
            while process.poll() is None:
                require(time.monotonic() < deadline, "Burn release verifier timed out")
                require(os.fstat(stdout.fileno()).st_size <= MAX_OUTPUT and os.fstat(stderr.fileno()).st_size <= MAX_OUTPUT,
                        "Burn release verifier output bound")
                time.sleep(0.05)
            stdout.seek(0); stderr.seek(0)
            out, err = stdout.read(MAX_OUTPUT + 1), stderr.read(MAX_OUTPUT + 1)
            require(len(out) <= MAX_OUTPUT and len(err) <= MAX_OUTPUT, "Burn release verifier output bound")
            return subprocess.CompletedProcess(arguments, process.returncode, out, err)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


def bind_fresh_outputs(report, paths, output, receipt):
    """Close every fresh verifier reference inside the outer content bundle."""
    import archive_verified_burn_evidence as archive
    import release_package as package
    require(not any(row["role"].startswith(GENERATED_PREFIX) or
                    row.get("sourcePath", "").startswith(GENERATED_PATH) for row in report["artifacts"]),
            "Reserved generated Burn evidence role/path collision")
    result = receipt["result"]
    expected = {
        "Stdout": receipt["stdoutSha256"], "Stderr": receipt["stderrSha256"],
        "Receipt": receipt["completionReceiptSha256"],
        "CanonicalReceipts": result["canonicalReceiptsSha256"],
        "NativeReceipt": result["nativeVerificationSha256"],
        "RederivedLedger": result["rederivedLedgerSha256"],
        "GateReceipt": hashlib.sha256((json.dumps(receipt, sort_keys=True, indent=2) + "\n").encode()).hexdigest(),
    }
    archived = output / "verified-history-archive"
    archived_report = archive.archive(output / "fresh-execution/verified-history/verification-complete.json", archived)
    require(archived_report["sourceManifestSha256"] == result["nativeVerificationSha256"],
            "Native Burn completion changed before evidence closure")
    index = archived / archive.INDEX_NAME
    # archive() already independently verifies every member/reference. Retain
    # those exact digests; don't re-admit changed bytes during outer packaging.
    expected["HistoryArchive"] = hashlib.sha256(archive.archive_tools.canonical(archived_report)).hexdigest()
    expected["HistoryArchiveComplete"] = hashlib.sha256((expected["HistoryArchive"] + "\n").encode()).hexdigest()
    generated = {
        "Stdout": output / "stdout.json", "Stderr": output / "stderr.log",
        "Receipt": output / "fresh-execution/burn-verification-complete.json",
        "CanonicalReceipts": output / "fresh-execution/canonical-receipts.json",
        "NativeReceipt": output / "fresh-execution/verified-history/verification-complete.json",
        "RederivedLedger": output / "fresh-execution/rederived-ledger.json",
        "GateReceipt": output / "BURN-GATE-COMPLETE.json",
        "HistoryArchive": index,
        "HistoryArchiveComplete": archived / "ARCHIVE-COMPLETE",
    }
    for index, shard in enumerate(archived_report["shards"]):
        suffix = f"HistoryShard{index:05d}"
        generated[suffix] = archived / shard["file"]
        expected[suffix] = shard["sha256"]
    require(len(report["artifacts"]) + len(generated) <= 16384, "Fresh Burn evidence exceeds outer artifact bound")
    rows = []
    for suffix, path in sorted(generated.items()):
        role = GENERATED_PREFIX + suffix
        require(role not in paths, "Reserved generated Burn evidence role collision")
        relative = path.relative_to(output).as_posix()
        path = package.local_file(output, relative)
        actual = package.digest_file(path)
        require(actual["sha256"] == expected[suffix], "Fresh Burn evidence changed before outer binding")
        rows.append({"role": role, **actual, "object": "objects/" + actual["sha256"],
                     "sourcePath": GENERATED_PATH + relative, "executable": False})
        paths[role] = path
    report["artifacts"].extend(rows)


def verify(report, paths, *, native, native_sha256, node, node_sha256, bundle_sha256, output, timeout=86400):
    import release_package as package
    bindings = bind_inputs(report, paths)
    for binary, digest, role in ((native, native_sha256, "burnNativeVerifier"), (node, node_sha256, "burnNodeBinary")):
        require(isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest) and binary.is_file() and not binary.is_symlink() and
                package.digest_file(binary)["sha256"] == digest == bindings[role], "Explicit Burn executable pin differs from release artifact")
    require(isinstance(bundle_sha256, str) and re.fullmatch(r"[0-9a-f]{64}", bundle_sha256) and
            bundle_sha256 == bindings["burnRecoveryVerifier"], "Explicit bundled Burn verifier pin differs")
    require(type(timeout) is int and 1 <= timeout <= 7 * 86400, "Burn verification timeout bound")
    require(not output.exists(), "Burn verification output must be a new directory")
    output.mkdir(parents=True)
    manifest = paths["burnHistoryManifest"]
    history = scale.read_json(manifest)
    if history.get("schema") == "sora-nexus-burn-evidence-archive-v1":
        import archive_burn_evidence
        restored = archive_burn_evidence.restore(manifest, output / "raw-history")
        manifest = Path(restored["manifest"])
        history = scale.read_json(manifest)
    require(history.get("schema") == "sora-nexus-burn-proof-stream-v1" and history.get("first") == FIRST and
            history.get("last") == scale.CHECKPOINT and history.get("checkpointHash") == scale.CHECKPOINT_HASH,
            "Complete checkpoint Burn history is required; diagnostic ranges cannot clear the gate")
    manifest_sha256 = package.digest_file(manifest)["sha256"]
    arguments = [str(paths["burnRecoveryVerifier"]), "--verifier", str(native.resolve()), "--verifier-sha256", native_sha256,
                 "--manifest", str(manifest.resolve()), "--policy", str(paths["burnAllocationPolicy"]),
                 "--policy-sha256", bindings["burnAllocationPolicy"], "--expected-ledger", str(paths["burnEntitlements"]),
                 "--output", str((output / "fresh-execution").resolve())]
    completed = execute(node.resolve(), arguments, timeout)
    receipt = {"nodeSha256": node_sha256, "bundledVerifierSha256": bundle_sha256, "nativeVerifierSha256": native_sha256,
               "inputSha256": {role: bindings[role] for role in sorted(INPUTS)}, "arguments": arguments,
               "exitCode": completed.returncode, "stdoutSha256": hashlib.sha256(completed.stdout).hexdigest(),
               "stderrSha256": hashlib.sha256(completed.stderr).hexdigest()}
    (output / "stdout.json").write_bytes(completed.stdout)
    (output / "stderr.log").write_bytes(completed.stderr)
    require(completed.returncode == 0, "Fresh complete Burn verification failed; see " + str(output / "stderr.log"))
    result_path = output / "fresh-execution" / "burn-verification-complete.json"
    receipt["completionReceiptSha256"] = package.digest_file(result_path)["sha256"]
    result = scale.read_json(result_path)
    require(package.digest_file(result_path)["sha256"] == receipt["completionReceiptSha256"],
            "Fresh Burn completion changed during validation")
    require(json.loads(completed.stdout) == result, "Burn verifier stdout differs from fresh completion receipt")
    require(result.get("schema") == "sora-nexus-burn-release-verification-v1" and
            result.get("checkpoint") == {"block": scale.CHECKPOINT, "hash": scale.CHECKPOINT_HASH} and
            all(result.get(field) == digest for field, digest in (
                ("sourceManifestSha256", manifest_sha256), ("allocationPolicySha256", bindings["burnAllocationPolicy"]),
                ("expectedLedgerSha256", bindings["burnEntitlements"]), ("rederivedLedgerSha256", bindings["burnEntitlements"]),
                ("nativeVerifierSha256", native_sha256))) and
            all(result.get(field) is True for field in ("completeHistoryFreshlyVerified", "sharedEventExtractorExecuted",
                "exactAllocationRederived", "ledgerBytesIdentical", "replacementOwnershipAppliedAfterAllocation",
                "attackerCurvePreserved", "mappedNexusRecipientsHeld")) and
            all(result.get(field) is False for field in ("consensusFinalityVerified", "claimServiceDeploymentVerified", "productionReady")),
            "Fresh Burn verifier receipt is not bound to the admitted release")
    for name, field in (("canonical-receipts.json", "canonicalReceiptsSha256"),
                        ("verified-history/verification-complete.json", "nativeVerificationSha256"),
                        ("rederived-ledger.json", "rederivedLedgerSha256")):
        require(package.digest_file(output / "fresh-execution" / name)["sha256"] == result.get(field),
                "Fresh derived Burn evidence digest mismatch")
    require((output / "fresh-execution/rederived-ledger.json").read_bytes() == paths["burnEntitlements"].read_bytes(),
            "Packaged Burn ledger differs from freshly rederived bytes")
    bind_inputs(report, paths)
    require(package.digest_file(native)["sha256"] == native_sha256 and
            package.digest_file(node)["sha256"] == node_sha256,
            "Explicit Burn executable changed during verification")
    receipt["result"] = result
    (output / "BURN-GATE-COMPLETE.json").write_text(json.dumps(receipt, sort_keys=True, indent=2) + "\n")
    bind_fresh_outputs(report, paths, output, receipt)
    report["gates"][GATE] = {"status": "verified", "reason": "Fresh complete authenticated event extraction and exact ledger rederivation; claim-service deployment and consensus finality remain independent", "receipt": receipt}
    report["productionReady"] = False
    return receipt
