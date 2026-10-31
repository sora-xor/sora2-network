#!/usr/bin/env python3
"""Run local Wasm rehearsals with fresh, cryptographically linked evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from uuid import uuid4

ROOT = Path(__file__).resolve().parent
VALIDATION = ROOT / "validation"


def now():
    return datetime.now(timezone.utc).isoformat()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_text())


def supporting_evidence(validation, repository):
    """Bind imported audit/review results to unchanged files at final execution."""
    files = []
    imported = validation / "imported-migration-evidence"
    summary_path = imported / "import-summary.json"
    summary = read(summary_path)
    files.append(summary_path)
    assert summary["status"] == "passed" and summary["cargoAuditCount"] == 11
    assert summary["cargoVulnerabilities"] == summary["cargoWarnings"] == 0
    for row in summary["files"]:
        path = (imported / row["path"]).resolve()
        assert path.is_relative_to(imported.resolve())
        assert digest(path) == row["sha256"] == row["sourceSha256"]
        files.append(path)
    for name, row in summary["cargoAuditedLockfiles"].items():
        path = (repository / name).resolve()
        assert path.is_relative_to(repository.resolve())
        assert digest(path) == row["sha256"], "Audited Cargo lock changed: " + name
        files.append(path)
    source_files = dict(summary["nodeClientSourceFiles"])
    assert summary["regressionHarnessAuditCount"] == 5
    source_files.update(summary["additionalHarnessSourceFiles"])
    source_files[summary["llvmWrapperSource"]["path"]] = summary["llvmWrapperSource"]["sha256"]
    for package, row in read(imported / "node-sdk-source-rebase.json").items():
        if package == "node":
            continue
        source_files["vendor/polkadot-sdk/" + package + "/Cargo.toml"] = row["manifest_sha256"]
    for name, expected in source_files.items():
        path = (repository / name).resolve()
        assert path.is_relative_to(repository.resolve())
        assert digest(path) == expected, "Reviewed source changed: " + name
        files.append(path)
    npm_dir = validation / "npm-security-refresh"
    npm_summary = read(npm_dir / "summary.json")
    npm_installed = read(npm_dir / "installed-checks.json")
    assert npm_summary["status"] == npm_installed["status"] == "passed"
    assert len(npm_summary["projects"]) == 7
    npm_helper = repository / "misc/dependency_security/prepare_npm_audit.py"
    assert digest(npm_helper) == npm_summary["helper_sha256"]
    files.append(npm_helper)
    for project in npm_summary["projects"]:
        assert project["closure_exit"] == project["audit_exit"] == project["vulnerabilities"]["total"] == 0
        for name, key in (("manifest", "manifest_sha256"), ("lockfile", "lock_sha256")):
            path = (repository / project["project"] / project[name]).resolve()
            assert path.is_relative_to(repository.resolve())
            assert digest(path) == project[key], "Audited npm input changed: " + str(path)
            files.append(path)
    for check in npm_installed["checks"]:
        assert check["exit"] == 0
        path = (repository / check["log"]).resolve()
        assert path.is_relative_to(npm_dir.resolve()) and digest(path) == check["log_sha256"]
    files.extend(path for path in npm_dir.rglob("*") if path.is_file())
    identity_path = validation / "current-production-identity.json"
    identity = read(identity_path)
    assert identity["status"] == "passed" and identity["pinnedRuntimeStillRepresentative"], "Current finalized runtime no longer matches the pinned predecessor"
    assert identity["snapshotPreserved"] and identity["pinnedBlockRemainsCanonical"]
    assert identity["inputs"]["snapshotSha256"] == digest(validation / "snapshot.json")
    assert identity["inputs"]["scriptSha256"] == digest(validation / "check-current-production.cjs")
    assert identity["inputs"]["rpcGuardSha256"] == digest(validation / "readonly-rpc.cjs")
    files += [identity_path, validation / "check-current-production.cjs"]
    return files, {"cargoLockfiles": 11, "regressionHarnessLockfiles": 5, "npmGraphs": 7,
                   "currentFinalizedRuntimeMatchesPinned": True,
                   "currentIdentityBlock": identity["currentFinalized"]["blockNumber"],
                   "importedReviewSha256": digest(summary_path),
                   "npmAuditSha256": digest(npm_dir / "summary.json"),
                   "currentIdentitySha256": digest(identity_path)}


def validate_build_evidence(proof, candidate_hash, validation):
    validation = validation.resolve()
    assert proof["status"] == "passed", "Build evidence is not passed"
    assert proof["candidateSha256"] == candidate_hash, "Build evidence must bind exact candidate bytes"
    assert proof.get("buildCommand") and proof.get("sourceInputs"), "Build command and source inputs are required"
    def local(relative):
        path = (validation / relative).resolve()
        assert path.is_relative_to(validation), "Build evidence references must stay inside validation"
        return path
    fingerprint_ref = proof["sourceFingerprint"]
    fingerprint_path = local(fingerprint_ref["path"])
    assert digest(fingerprint_path) == fingerprint_ref["sha256"], "Source fingerprint file changed"
    fingerprint = read(fingerprint_path)
    assert fingerprint["status"] == "final"
    assert fingerprint["source"]["sha256"] == fingerprint_ref["sourceSha256"], "Source aggregate mismatch"
    assert fingerprint["source"]["fileCount"] == fingerprint_ref["sourceFiles"]
    verification_path = local(proof["verificationReport"])
    verification = read(verification_path)
    assert verification["status"] == "passed", "Final build verification is not passed"
    assert verification["candidate"] == proof, "Build verification does not identify this candidate evidence"
    assert verification["sourceFingerprint"] == fingerprint_ref
    required = {"source-start", "clippy-all", "source-after-clippy", "format", "production-feature-graph", "production-wasm", "source-after-build"}
    assert required <= {row["name"] for row in verification["steps"]}, "Final build verification is missing required gates"
    evidence_files = [fingerprint_path, verification_path]
    for row in verification["steps"]:
        assert row["exitCode"] == 0, "Failed build verification step: " + row["name"]
        for stream in ("stdout", "stderr"):
            path = local(row[stream])
            assert digest(path) == row[stream + "Sha256"], "Build log changed: " + row["name"]
            evidence_files.append(path)
    for name in ("source-clippy-end-check.json", "source-build-end-check.json"):
        path = local(name)
        checked = read(path)
        assert checked["status"] == "passed" and checked["changes"] == [], "Build source changed: " + name
        assert checked["compared"] == "source" and checked["sourceSha256"] == fingerprint_ref["sourceSha256"]
        evidence_files.append(path)
    recorder = validation / "source-fingerprint.py"
    assert digest(recorder) == fingerprint["recorderSha256"], "Source fingerprint recorder changed"
    evidence_files.append(recorder)
    return fingerprint_path, evidence_files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--build-evidence", type=Path, required=True)
    parser.add_argument("--node", default="node")
    parser.add_argument("--signing-python", default=os.environ.get("REHEARSAL_SIGNING_PYTHON", "/tmp/sora2-runtime-upgrade-safe-python313/bin/python"))
    parser.add_argument("--candidate-spec", type=int, default=136)
    options = parser.parse_args()
    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ-") + uuid4().hex[:8]
    report_path = VALIDATION / "rehearsal-run.json"
    archive = VALIDATION / "attempts" / ("prior-to-" + run_id)
    current_reports = ["rehearsal-run.json", "wasm-compatibility.json", "wasm-compatibility.log",
        "baseline-metadata.json", "candidate-metadata.json", "candidate-metadata.scale", "metadata-compatibility.json", "python-candidate-signing-report.json", "python-candidate-signing.log",
        "current-production-identity.json", "current-production-identity.log",
        "equivocation-fee-wasm-rehearsal.json", "equivocation-fee-wasm-rehearsal.log",
        "fee-event-check.json", "fee-event-check.log", "fee-policy-wasm-rehearsal.json",
        "fee-policy-wasm-rehearsal.log", "bridge-readiness.json", "bridge-readiness.log",
        "bridge-baseline-metadata.json", "wasm-interface-tests.log", "readonly-rpc-tests.log",
        "fee-evidence-tests.log", "rehearsal-run-tests.log", "runtime-execution-tests.log", "sdk2609-compatibility-tests.log", "authorized-heartbeat-tests.log", "sdk-patches-tests.log", "sdk-primitives-provenance.json", "sdk-primitives-provenance.log", "source-before-rehearsal.json", "source-before-rehearsal.log", "source-after-rehearsal.json", "source-after-rehearsal.log"]
    archived = []
    for name in current_reports:
        source = VALIDATION / name
        if source.exists():
            archive.mkdir(parents=True, exist_ok=True)
            target = archive / name
            source.rename(target)
            archived.append({"path": str(target.relative_to(ROOT)), "sha256": digest(target)})
    report = {"runId": run_id, "status": "running", "startedAt": now(),
        "scope": "Read-only production RPC; candidate execution and fixtures remain local. No transaction submission or deployment.",
        "archivedPriorOutputs": archived, "stages": []}

    def save():
        temporary = report_path.with_suffix(".tmp")
        temporary.write_text(json.dumps(report, indent=2) + "\n")
        temporary.replace(report_path)

    def stage(name, command, log_name, result_name=None):
        row = {"name": name, "status": "running", "startedAt": now(), "command": command}
        report["stages"].append(row)
        save()
        log = VALIDATION / log_name
        with log.open("w") as output:
            process = subprocess.run(command, cwd=ROOT.parent, stdout=output, stderr=subprocess.STDOUT)
        row.update(exitCode=process.returncode, finishedAt=now(), log=str(log.relative_to(ROOT)), logSha256=digest(log))
        if process.returncode:
            row["status"] = "failed"
            raise RuntimeError(f"Stage {name} failed with exit {process.returncode}; see {log}")
        if result_name:
            path = VALIDATION / result_name
            result = read(path)
            assert result["status"] == "passed", f"Stage {name} did not produce passing evidence"
            row.update(result=str(path.relative_to(ROOT)), resultSha256=digest(path))
        row["status"] = "passed"
        save()

    save()
    try:
        if not __debug__:
            raise RuntimeError("Rehearsal verification requires Python assertions; do not use -O/PYTHONOPTIMIZE")
        source = options.candidate.resolve()
        candidate_hash = digest(source)
        proof = read(options.build_evidence)
        fingerprint_path, build_evidence_files = validate_build_evidence(proof, candidate_hash, VALIDATION)
        proof_copy = VALIDATION / "candidate-build-evidence.json"
        if options.build_evidence.resolve() != proof_copy:
            shutil.copyfile(options.build_evidence, proof_copy)
        assert isinstance(proof["sourceInputs"], dict)
        source_files = []
        for relative, expected in proof["sourceInputs"].items():
            source_path = (ROOT.parent / relative).resolve()
            assert source_path.is_relative_to(ROOT.parent), "Build source input must stay inside repository"
            assert digest(source_path) == expected, f"Build source input changed: {relative}"
            source_files.append(source_path)
        candidate = ROOT / "candidate.compact.compressed.wasm"
        if candidate.exists() and digest(candidate) != candidate_hash:
            archive.mkdir(parents=True, exist_ok=True)
            candidate.rename(archive / candidate.name)
            report["archivedPriorOutputs"].append({"path": str((archive / candidate.name).relative_to(ROOT)), "sha256": digest(archive / candidate.name)})
        if not candidate.exists():
            shutil.copyfile(source, candidate)
        assert digest(candidate) == candidate_hash
        snapshot = VALIDATION / "snapshot.json"
        snapshot_data = read(snapshot)
        assert snapshot_data["status"] == "passed"
        assert snapshot_data["endpoint"] == "wss://mof2.sora.org"
        assert options.candidate_spec > snapshot_data["runtimeVersion"]["specVersion"]
        stage("Current finalized production runtime identity", [options.node, str(VALIDATION / "check-current-production.cjs")], "current-production-identity.log", "current-production-identity.json")
        bindings = [ROOT / "run-local-rehearsals.py", ROOT / "run-local-rehearsals.sh"]
        bindings += [VALIDATION / file for file in ["verify-wasm-compatibility.cjs", "wasm-interfaces.cjs", "test-wasm-interfaces.cjs", "readonly-rpc.cjs", "test-readonly-rpc.cjs", "validate-dependencies.cjs", "compare-metadata.py", "verify-fee-events.py", "test-fee-evidence.py", "test-rehearsal-run.py", "runtime-execution.cjs", "test-runtime-execution.cjs", "rehearse-equivocation-fees.cjs", "rehearse-fee-policy.cjs", "check-bridge-readiness.cjs", "retired-lending-fixture.cjs", "payout-batch-fixture.cjs", "migration-proof-fixture.json", "package.json", "package-lock.json"]]
        bindings += build_evidence_files
        bindings += [VALIDATION / "test-sdk2609-compatibility.py"]
        bindings += [VALIDATION / "authorized-heartbeat-fixture.cjs"]
        bindings += [VALIDATION / "test-authorized-heartbeat.cjs"]
        bindings += [VALIDATION / "verify-sdk-patches.py", VALIDATION / "test-sdk-patches.py"]
        bindings += [VALIDATION / "check-python-candidate-signing.py"]
        supporting_files, supporting_checks = supporting_evidence(VALIDATION, ROOT.parent)
        bindings += supporting_files
        bindings += source_files
        bindings += [ROOT.parent / file for file in ["Cargo.lock", "rust-toolchain.toml", "runtime/src/lib.rs"]]
        report["inputs"] = {"candidateSha256": candidate_hash, "candidateSourcePath": str(source), "candidatePath": str(candidate), "snapshotSha256": digest(snapshot), "snapshotBlockHash": snapshot_data["blockHash"], "buildEvidenceSha256": digest(proof_copy), "files": {str(file.relative_to(ROOT.parent)): digest(file) for file in bindings}}
        save()
        node = options.node
        stage("Verify full source before rehearsal", [sys.executable, str(VALIDATION / "source-fingerprint.py"), "--check", str(fingerprint_path), "--source-only", "--output", str(VALIDATION / "source-before-rehearsal.json")], "source-before-rehearsal.log", "source-before-rehearsal.json")
        stage("Wasm interface negative regressions", [node, "--test", str(VALIDATION / "test-wasm-interfaces.cjs")], "wasm-interface-tests.log")
        stage("Read-only RPC guard regressions", [node, "--test", str(VALIDATION / "test-readonly-rpc.cjs")], "readonly-rpc-tests.log")
        stage("Fee evidence binding regressions", [sys.executable, str(VALIDATION / "test-fee-evidence.py")], "fee-evidence-tests.log")
        stage("Run evidence negative regressions", [sys.executable, str(VALIDATION / "test-rehearsal-run.py")], "rehearsal-run-tests.log")
        stage("Runtime lifecycle and rollback regressions", [node, "--test", str(VALIDATION / "test-runtime-execution.cjs")], "runtime-execution-tests.log")
        stage("SDK2609 compatibility exception regressions", [sys.executable, str(VALIDATION / "test-sdk2609-compatibility.py")], "sdk2609-compatibility-tests.log")
        stage("Authorized heartbeat fixture encoding regressions", [node, "--test", str(VALIDATION / "test-authorized-heartbeat.cjs")], "authorized-heartbeat-tests.log")
        stage("SDK source provenance negative regressions", [sys.executable, str(VALIDATION / "test-sdk-patches.py")], "sdk-patches-tests.log")
        stage("SDK primitives source reconstruction", [sys.executable, str(VALIDATION / "verify-sdk-patches.py")], "sdk-primitives-provenance.log", "sdk-primitives-provenance.json")
        stage("Exact candidate compatibility", [node, str(VALIDATION / "verify-wasm-compatibility.cjs"), "--candidate", str(candidate), "--candidate-spec", str(options.candidate_spec), "--baseline", str(VALIDATION / "live-runtime.compact.compressed.wasm"), "--output-dir", str(VALIDATION)], "wasm-compatibility.log", "wasm-compatibility.json")
        stage("Offline candidate governance signing compatibility", [options.signing_python, str(VALIDATION / "check-python-candidate-signing.py"), "--metadata", str(VALIDATION / "candidate-metadata.scale"), "--runtime-report", str(VALIDATION / "wasm-compatibility.json"), "--out", str(VALIDATION / "python-candidate-signing-report.json")], "python-candidate-signing.log", "python-candidate-signing-report.json")
        common = ["--endpoint", snapshot_data["endpoint"], "--wasm", str(candidate), "--candidate-spec", str(options.candidate_spec), "--synthetic-fixtures", "--snapshot", str(snapshot), "--cache", str(VALIDATION / ".public-read-cache.json.gz")]
        stage("Equivocation and fee execution", [node, str(VALIDATION / "rehearse-equivocation-fees.cjs"), *common, "--output", str(VALIDATION / "equivocation-fee-wasm-rehearsal.json")], "equivocation-fee-wasm-rehearsal.log", "equivocation-fee-wasm-rehearsal.json")
        stage("Independent fee event verification", [sys.executable, str(VALIDATION / "verify-fee-events.py")], "fee-event-check.log", "fee-event-check.json")
        stage("Fee policy and bridge fixtures", [node, str(VALIDATION / "rehearse-fee-policy.cjs"), *common, "--output", str(VALIDATION / "fee-policy-wasm-rehearsal.json")], "fee-policy-wasm-rehearsal.log", "fee-policy-wasm-rehearsal.json")
        stage("Complete pending bridge inventory", [node, str(VALIDATION / "check-bridge-readiness.cjs")], "bridge-readiness.log", "bridge-readiness.json")
        stage("Verify full source after rehearsal", [sys.executable, str(VALIDATION / "source-fingerprint.py"), "--check", str(fingerprint_path), "--source-only", "--output", str(VALIDATION / "source-after-rehearsal.json")], "source-after-rehearsal.log", "source-after-rehearsal.json")
        compatibility = read(VALIDATION / "wasm-compatibility.json")
        equivocation = read(VALIDATION / "equivocation-fee-wasm-rehearsal.json")
        fees = read(VALIDATION / "fee-event-check.json")
        policy = read(VALIDATION / "fee-policy-wasm-rehearsal.json")
        bridge = read(VALIDATION / "bridge-readiness.json")
        provenance = read(VALIDATION / "sdk-primitives-provenance.json")
        signing = read(VALIDATION / "python-candidate-signing-report.json")
        assert signing["candidate_sha256"] == candidate_hash
        assert signing["runtime_report_sha256"] == digest(VALIDATION / "wasm-compatibility.json")
        assert signing["metadata_sha256"] == compatibility["inputs"]["candidate"]["metadataScaleSha256"]
        assert len(signing["cases"]) == 8 and signing["rpc_calls"] == signing["submission_calls"] == 0
        assert provenance["status"] == "passed" and len(provenance["packages"]) == 7
        assert policy["checks"]["authorizedHeartbeatMigration"]
        heartbeats = policy["authorizedHeartbeatChecks"]
        assert len(heartbeats) == 7 and all(row["passed"] for row in heartbeats)
        assert all(set(row["sources"]) == {"External", "Local", "InBlock"} for row in heartbeats)
        assert policy["checks"]["emptyUtilityBatchesAreNormalAndPaid"]
        for kind in ("batch", "batchAll", "forceBatch"):
            rows = [row for row in policy["signedChecks"] if row["method"] == "utility." + kind + " empty Normal paid"]
            assert len(rows) == 1 and rows[0]["passed"] and rows[0]["dispatchClass"] == "Normal"
            assert int(rows[0]["xorCharged"]) > 0
        assert digest(candidate) == candidate_hash == compatibility["inputs"]["candidate"]["sha256"] == equivocation["candidate"]["sha256"] == policy["candidate"]["sha256"] == fees["candidateSha256"]
        assert digest(snapshot) == report["inputs"]["snapshotSha256"] == compatibility["snapshotSha256"] == equivocation["inputs"]["snapshotSha256"] == policy["inputs"]["snapshotSha256"] == fees["snapshotSha256"] == bridge["snapshotSha256"]
        assert bridge["pendingInventoryComplete"] and bridge["fullPendingInventoryGatePassed"]
        assert fees["compatibilitySha256"] == digest(VALIDATION / "wasm-compatibility.json")
        assert fees["rehearsalSha256"] == digest(VALIDATION / "equivocation-fee-wasm-rehearsal.json")
        for file, expected in report["inputs"]["files"].items():
            assert digest(ROOT.parent / file) == expected, f"Input changed during run: {file}"
        assert digest(proof_copy) == report["inputs"]["buildEvidenceSha256"]
        report["checks"] = {"freshStageOutputs": True, "allRequiredStagesPassed": True, "candidateSnapshotAndEvidenceHashesAgree": True, "scriptAndSourceBindingsUnchangedDuringRun": True, "fullPendingInventoryComplete": True}
        report["sdkMigration"] = {
            "reviewedCompatibility": compatibility["checks"]["reviewedSdkMigrationCompatible"],
            "existingScaleEncodingCompatible": compatibility["checks"]["existingScaleEncodingCompatible"],
            "codeUpdatedEventRequiresMetadataRefresh": True,
            "reconstructedPrimitivePackages": len(provenance["packages"]),
            "sourceProvenanceSha256": digest(VALIDATION / "sdk-primitives-provenance.json"),
            "authorizedHeartbeatCases": len(heartbeats),
            "authorizedHeartbeatAdmissionSourcesPerCase": 3,
            "emptyUtilityBatchesNormalAndPaid": 3,
            "offlineGovernanceSigningCases": len(signing["cases"]),
            "signedHeartbeatStillPaid": any(row["method"] == "imOnline.heartbeat signed outer transaction remains paid BadOrigin"
                and row["passed"] and int(row["xorCharged"]) > 0 for row in policy["signedChecks"]),
        }
        report["supportingEvidence"] = supporting_checks
        assert report["sdkMigration"]["signedHeartbeatStillPaid"]
        report["status"] = "passed"
    except BaseException as error:
        report["status"] = "failed"
        report["error"] = {"type": type(error).__name__, "message": str(error)}
    finally:
        report["finishedAt"] = now()
        save()
        print(json.dumps({"status": report["status"], "runId": run_id, "report": str(report_path), "error": report.get("error")}))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
