"""Bounded incoming-coverage inputs; never authenticate external finality here."""
import re
import incoming_replay_release

import hashi_codec as scale

ROLES = {"incomingDeliveryInventory", "recoveryWorkIndex"}
INPUTS = ROLES | {"checkpointCaptureManifest", "policyScale", "incomingReconciliation", "externalFinalizedHeads"}
MAX_SHARD = 576 * 1024
MAX_TOTAL = 512 * 1024 * 1024


def bind_work(root, records, paths, policy, local_file, digest_file):
    if "recoveryWorkIndex" not in paths:
        return
    source = paths["recoveryWorkIndex"]
    index = scale.read_json(source)
    scale.require(isinstance(index, dict) and set(index) in ({"schema", "policySha256", "work", "audits", "terminal"}, {"schema", "policySha256", "work", "audits", "terminal", "terminalPrelude"})
                  and index["schema"] == "sora-nexus-recovery-work-index-v1", "Invalid recovery work index")
    scale.require(isinstance(index["policySha256"], str) and re.fullmatch(r"[0-9a-f]{64}", index["policySha256"]), "Invalid work policy hash")
    policy_role = "incomingActivePolicyScale" if "incomingActivePolicyScale" in paths else "policyScale"
    if policy_role in records:
        scale.require(index["policySha256"] == records[policy_role]["sha256"], "Recovery work index policy mismatch")
    for field in ("work", "audits"):
        scale.require(isinstance(index[field], list) and 0 < len(index[field]) <= 16_384, "Recovery work shard count bound")
    if policy:
        scale.require(len(index["work"]) == policy["workChunks"] and len(index["audits"]) == policy["auditChunks"], "Incomplete policy work/audit shard coverage")
    prelude = index.get("terminalPrelude", [])
    scale.require(isinstance(prelude, list) and len(prelude) < 1024, "Recovery terminal page bound")
    references = [*index["work"], *index["audits"], *prelude, index["terminal"]]
    scale.require(len(references) + len(records) <= 16_384, "Recovery shard artifact inventory bound")
    indexed = {path: role for role, path in paths.items()}
    seen, total = set(), 0
    for position, reference in enumerate(references):
        scale.require(isinstance(reference, dict) and set(reference) == {"file", "bytes", "sha256"}, "Unexpected recovery work shard fields")
        scale.require(type(reference["bytes"]) is int and 0 < reference["bytes"] <= MAX_SHARD, "Recovery work shard size bound")
        scale.require(isinstance(reference["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", reference["sha256"]), "Invalid work shard hash")
        item = local_file(source.parent, reference["file"])
        scale.require(item not in seen, "Duplicate recovery work shard path")
        seen.add(item)
        total += reference["bytes"]
        scale.require(total <= MAX_TOTAL, "Recovery work aggregate bound")
        actual = digest_file(item)
        scale.require(actual["bytes"] == reference["bytes"] and actual["sha256"] == reference["sha256"], "Recovery work shard hash/size mismatch")
        if item in indexed:
            continue
        role = f"incomingWorkShard-{position:05d}"
        scale.require(role not in records, "Reserved incoming work shard role collision")
        records[role] = {"role": role, **actual, "object": "objects/" + actual["sha256"],
                         "sourcePath": item.relative_to(root).as_posix(), "executable": False}
        paths[role] = item


def arguments(paths):
    if not INPUTS <= paths.keys():
        return None
    arguments = ["verify-incoming-coverage", "--capture", str(paths["checkpointCaptureManifest"].parent),
            "--policy", str(paths["incomingActivePolicyScale"] if "incomingActivePolicyScale" in paths else paths["policyScale"]), "--work", str(paths["recoveryWorkIndex"]),
            "--inventory", str(paths["incomingDeliveryInventory"]), "--reconciliation", str(paths["incomingReconciliation"]),
            "--external-heads", str(paths["externalFinalizedHeads"])]
    if "recoveryParentReplay" in paths:
        arguments += ["--recovery-replay", str(paths["recoveryParentReplay"])]
    return arguments


def valid_receipt(result, report, bindings, checkpoint_root):
    if not isinstance(result, dict) or not report.get("policy"):
        return False
    policy = report.get("incomingRecoveryReplay", {}).get("activePolicy", report["policy"])
    policy_role = "incomingActivePolicyScale" if "incomingRecoveryReplay" in report else "policyScale"
    metadata = result.get("checkpointMetadata")
    evidence = result.get("evidenceHashes")
    bundled = {"0x" + row["blake2_256"] for row in report["artifacts"]}
    return bool(result.get("schema") == "sora-nexus-incoming-coverage-verification-v1"
        and result.get("checkpointHash") == scale.CHECKPOINT_HASH
        and result.get("checkpointStateRoot") == checkpoint_root
        and result.get("policyCommitment") == "0x" + policy["commitment"]
        and all(result.get(field) == bindings[role] for field, role in (
            ("captureManifestSha256", "checkpointCaptureManifest"), ("policySha256", policy_role),
            ("incomingWorkSha256", "recoveryWorkIndex"), ("incomingInventorySha256", "incomingDeliveryInventory"),
            ("incomingReconciliationSha256", "incomingReconciliation"), ("externalFinalizedHeadsSha256", "externalFinalizedHeads")))
        and all(result.get(field) is True for field in ("localPendingCoverageVerified", "declaredExternalInventoryCoverageVerified", "workCoverageVerified"))
        and all(result.get(field) is False for field in ("externalArchiveCompletenessVerified", "externalFinalityVerified", "authenticationVerified", "exactArtifactExecutionVerified", "productionReady"))
        and isinstance(metadata, dict) and metadata.get("schema") == "sora-nexus-checkpoint-metadata-witness-v1"
        and metadata.get("checkpointHash") == scale.CHECKPOINT_HASH and metadata.get("stateRoot") == checkpoint_root
        and metadata.get("wasmExecutionVerified") is True and metadata.get("nativeExecution") is False
        and isinstance(evidence, list) and all(isinstance(item, str) for item in evidence)
        and len(evidence) == len(set(evidence)) and set(evidence) <= bundled
        and incoming_replay_release.valid(result, report, bindings, checkpoint_root))
