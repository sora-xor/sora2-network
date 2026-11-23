#!/usr/bin/env python3
"""Run local Wasm rehearsals with fresh, cryptographically linked evidence."""
import argparse
import hashlib
import importlib.util
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


def load_module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def validate_current_cargo_audits(validation, repository):
    collector_path = validation / "capture-supporting-evidence.py"
    collector = load_module(collector_path, "supporting_evidence_collector")
    path = validation / "current-cargo-audits/summary.json"
    report = read(path)
    assert report["status"] == "passed" and report["collectorSha256"] == digest(collector_path)
    database_path = (repository / report["databaseSource"]).resolve()
    assert database_path.is_relative_to((validation / "current-cargo-audits").resolve())
    assert digest(database_path) == report["databaseSourceSha256"]
    database = read(database_path)
    assert database["status"] == "passed" and database["unchanged"] and database["before"] == database["after"]
    assert database["before"]["clean"] and database["before"]["trackedFiles"] > 0
    assert len(database["before"]["trackedBytesSha256"]) == 64
    assert len(report["databaseCommit"]) == 40 and database["before"]["commit"] == report["databaseCommit"]
    assert report["repositoryLockfiles"] == 11 and report["historicalHarnessLockfiles"] == 5
    expected = {name: "repository" for name in collector.LOCKFILES}
    imported = validation / "imported-migration-evidence"
    expected.update({str((imported / f"{name}-harness/Cargo.lock").relative_to(repository)): "historical-harness"
                     for name in collector.HARNESSES})
    rows = report["audits"]
    assert len(rows) == len(expected) == 16
    assert {row["lockfile"] for row in rows} == set(expected), "Fresh audit coverage changed"
    files = [path, collector_path, database_path]
    for row in rows:
        assert row["scope"] == expected[row["lockfile"]]
        assert row["exit"] == row["vulnerabilities"] == row["warnings"] == 0
        for field, sha in (("lockfile", "lockSha256"), ("audit", "auditSha256"), ("stderr", "stderrSha256")):
            source = (repository / row[field]).resolve()
            assert source.is_relative_to(repository.resolve())
            if field != "lockfile": assert source.is_relative_to((validation / "current-cargo-audits").resolve())
            assert digest(source) == row[sha], "Current Cargo audit input/output changed: " + row[field]
            files.append(source)
        audit = read(repository / row["audit"])
        assert audit["vulnerabilities"]["count"] == 0 and audit["vulnerabilities"]["list"] == []
        assert not any(audit["warnings"].values())
        assert audit["settings"]["ignore"] == [] and audit["settings"]["target_arch"] == [] and audit["settings"]["target_os"] == []
        assert row["databaseCommit"] == report["databaseCommit"]
        assert audit["database"]["last-commit"] == row["auditReportedDatabaseCommit"]
        assert row["auditReportedDatabaseCommit"] in (None, report["databaseCommit"])
        command = row["command"]
        assert len(command) == 10 and command[:3] == ["cargo", "audit", "--db"]
        assert Path(command[3]).resolve() == Path(database["before"]["path"]).resolve()
        assert command[4:9] == ["--no-fetch", "--deny", "warnings", "--json", "--file"]
        assert Path(command[9]).resolve() == (repository / row["lockfile"]).resolve()
    return files, report


def supporting_evidence(validation, repository):
    """Keep historical source review distinct from current dependency and runtime gates."""
    files, cargo = validate_current_cargo_audits(validation, repository)
    imported = validation / "imported-migration-evidence"
    summary_path = imported / "import-summary.json"
    summary = read(summary_path)
    historical = read(imported / "historical-reference.json")
    assert historical["status"] == "verified-historical-source-evidence"
    assert historical["summarySha256"] == digest(summary_path)
    assert summary["status"] == "passed" and summary["regressionHarnessAuditCount"] == 5
    files += [summary_path, imported / "historical-reference.json"]
    for row in summary["files"]:
        path = (imported / row["path"]).resolve()
        assert path.is_relative_to(imported.resolve())
        assert digest(path) == row["sha256"] == row["sourceSha256"]
        files.append(path)
    # Prior harness execution is relevant only while the reviewed package source
    # bytes remain unchanged. Its old root lock/manifest graph is not a claim
    # about the migrated runtime; the fresh 16 audits and full build cover that.
    source_files = dict(summary["nodeClientSourceFiles"])
    source_files.update(summary["additionalHarnessSourceFiles"])
    source_files[summary["llvmWrapperSource"]["path"]] = summary["llvmWrapperSource"]["sha256"]
    for package, row in read(imported / "node-sdk-source-rebase.json").items():
        if package != "node": source_files["vendor/polkadot-sdk/" + package + "/Cargo.toml"] = row["manifest_sha256"]
    for name, expected in source_files.items():
        path = (repository / name).resolve()
        assert path.is_relative_to(repository.resolve())
        assert digest(path) == expected, "Historically reviewed source changed: " + name
        files.append(path)
    npm_dir = validation / "npm-security-refresh"
    npm_summary = read(npm_dir / "summary.json")
    npm_installed = read(npm_dir / "installed-checks.json")
    assert npm_summary["status"] == npm_installed["status"] == "passed"
    npm_helper = repository / "misc/dependency_security/prepare_npm_audit.py"
    assert digest(npm_helper) == npm_summary["helper_sha256"]
    helper = load_module(npm_helper, "npm_audit_inventory")
    projects = npm_summary["projects"]
    assert len(projects) == len(helper.OPERATIONAL_PROJECTS) == 8
    assert {row["project"] for row in projects} == helper.OPERATIONAL_PROJECTS
    files.append(npm_helper)
    for project in projects:
        assert project["closure_exit"] == project["audit_exit"] == project["vulnerabilities"]["total"] == 0
        for name, key in (("manifest", "manifest_sha256"), ("lockfile", "lock_sha256")):
            path = (repository / project["project"] / project[name]).resolve()
            assert path.is_relative_to(repository.resolve())
            assert digest(path) == project[key], "Audited npm input changed: " + str(path)
            files.append(path)
        evidence = (repository / project["evidence"]).resolve()
        assert evidence.is_relative_to(npm_dir.resolve())
        assert digest(evidence / "audit.json") == project["audit_sha256"]
        assert digest(evidence / "closure.json") == project["closure_sha256"]
        assert read(evidence / "audit.json")["metadata"]["vulnerabilities"]["total"] == 0
        assert not read(evidence / "closure.json").get("problems")
    collector = load_module(validation / "capture-supporting-evidence.py", "installed_check_inventory")
    assert len(npm_installed["checks"]) == len(collector.INSTALLED_CHECKS)
    assert {row["name"] for row in npm_installed["checks"]} == collector.INSTALLED_CHECKS
    for check in npm_installed["checks"]:
        assert check["exit"] == 0
        for field, sha in (("log", "log_sha256"), ("stderr", "stderr_sha256")):
            path = (repository / check[field]).resolve()
            assert path.is_relative_to(npm_dir.resolve()) and digest(path) == check[sha]
    files.extend(path for path in npm_dir.rglob("*") if path.is_file())
    identity_path = validation / "current-production-identity.json"
    identity = read(identity_path)
    assert identity["status"] == "passed" and identity["pinnedRuntimeStillRepresentative"], "Current finalized runtime no longer matches the pinned predecessor"
    assert identity["snapshotPreserved"] and identity["pinnedBlockRemainsCanonical"]
    assert identity["inputs"]["snapshotSha256"] == digest(validation / "snapshot.json")
    assert identity["inputs"]["scriptSha256"] == digest(validation / "check-current-production.cjs")
    assert identity["inputs"]["rpcGuardSha256"] == digest(validation / "readonly-rpc.cjs")
    files += [identity_path, validation / "check-current-production.cjs"]
    return files, {"cargoLockfiles": 11, "regressionHarnessLockfiles": 5, "npmGraphs": 8,
                   "currentCargoAuditsSha256": digest(validation / "current-cargo-audits/summary.json"),
                   "rustSecDatabaseCommit": cargo["databaseCommit"],
                   "regressionHarnessExecution": "Historical SDK2609 tests; reviewed package sources unchanged; harness locks freshly audited",
                   "currentFinalizedRuntimeMatchesPinned": True,
                   "currentIdentityBlock": identity["currentFinalized"]["blockNumber"],
                   "historicalSourceReviewSha256": digest(summary_path),
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
    parser.add_argument("--candidate-spec", type=int, default=137)
    parser.add_argument("--upstream-repo", type=Path, default=Path(os.environ.get("SDK_UPSTREAM_REPO", "/tmp/sora2-stable2609-migration/upstream")))
    options = parser.parse_args()
    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ-") + uuid4().hex[:8]
    report_path = VALIDATION / "rehearsal-run.json"
    archive = VALIDATION / "attempts" / ("prior-to-" + run_id)
    current_reports = ["rehearsal-run.json", "wasm-compatibility.json", "wasm-compatibility.log",
        "baseline-metadata.json", "baseline-metadata.scale", "candidate-metadata.json", "candidate-metadata.scale", "metadata-compatibility.json", "python-candidate-signing-report.json", "python-candidate-signing.log",
        "current-production-identity.json", "current-production-identity.log",
        "equivocation-fee-wasm-rehearsal.json", "equivocation-fee-wasm-rehearsal.log",
        "fee-event-check.json", "fee-event-check.log", "fee-policy-wasm-rehearsal.json",
        "fee-policy-wasm-rehearsal.log", "bridge-readiness.json", "bridge-readiness.log",
        "bridge-baseline-metadata.json", "wasm-interface-tests.log", "readonly-rpc-tests.log",
        "fee-evidence-tests.log", "rehearsal-run-tests.log", "runtime-execution-tests.log", "sdk2609-compatibility-tests.log", "authorized-heartbeat-tests.log", "sdk-patches-tests.log", "sdk-primitives-provenance.json", "sdk-primitives-provenance.log", "source-before-rehearsal.json", "source-before-rehearsal.log", "source-after-rehearsal.json", "source-after-rehearsal.log"]
    current_reports += ["staking-async-source-proof.json", "staking-async-source-proof.log", "staking-async-source-tests.log", "bridge-readiness-policy-tests.log", "bridge-activation-current.json", "bridge-activation-current.log", "bridge-activation-snapshot.json", "bridge-readiness-verification.json", "bridge-readiness-verification.log", "bridge-cancellation-review.json", "bridge-payload-recovery-verification.json", "bridge-payload-recovery-verification.log"]
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
        bindings += [VALIDATION / name for name in ["staking-public-inventory.json", "staking-public-state.json.gz", "staking-cache-import.json", "capture-staking.cjs", "decode-staking.cjs"]]
        bindings += [VALIDATION / "test-sdk2609-compatibility.py"]
        bindings += [VALIDATION / "authorized-heartbeat-fixture.cjs"]
        bindings += [VALIDATION / "test-authorized-heartbeat.cjs"]
        bindings += [VALIDATION / "verify-sdk-patches.py", VALIDATION / "test-sdk-patches.py"]
        bindings += [VALIDATION / "verify-staking-async-source.py", VALIDATION / "test-staking-async-source.py"]
        bindings += [VALIDATION / "verify-staking-query-compatibility.cjs", VALIDATION / "test-staking-query-compatibility.cjs"]
        bindings += [VALIDATION / "check-python-candidate-signing.py"]
        bindings += [VALIDATION / name for name in ["bridge-readiness-policy.cjs", "test-bridge-readiness-policy.cjs", "verify-bridge-readiness.cjs", "bridge-rpc-size.cjs", "test-bridge-rpc-size.cjs", "verify-bridge-payload-recovery.cjs", "test-bridge-payload-recovery.cjs", "bridge-request-recovery-source.json", "bridge-archived-payload-source.json", "bridge-request-recovery-capture.json", "bridge-request-recovery-pages.jsonl.gz", "collect-bridge-payload-requests.cjs", "collect-bridge-payload-archive.cjs"]]
        if (VALIDATION / "bridge-payload-evidence.json").exists():
            bindings += [VALIDATION / "bridge-payload-evidence.json"]
        supporting_files, supporting_checks = supporting_evidence(VALIDATION, ROOT.parent)
        bindings += supporting_files
        bindings += source_files
        bindings += [ROOT.parent / file for file in ["Cargo.lock", "rust-toolchain.toml", "runtime/src/lib.rs"]]
        report["inputs"] = {"candidateSha256": candidate_hash, "candidateSourcePath": str(source), "candidatePath": str(candidate), "snapshotSha256": digest(snapshot), "snapshotBlockHash": snapshot_data["blockHash"], "buildEvidenceSha256": digest(proof_copy), "files": {str(file.relative_to(ROOT.parent)): digest(file) for file in bindings}}
        save()
        node = options.node
        stage("Verify full source before rehearsal", [sys.executable, str(VALIDATION / "source-fingerprint.py"), "--check", str(fingerprint_path), "--source-only", "--output", str(VALIDATION / "source-before-rehearsal.json")], "source-before-rehearsal.log", "source-before-rehearsal.json")
        stage("Wasm interface negative regressions", [node, "--test", str(VALIDATION / "test-wasm-interfaces.cjs"), str(VALIDATION / "test-staking-query-compatibility.cjs")], "wasm-interface-tests.log")
        stage("Read-only RPC guard regressions", [node, "--test", str(VALIDATION / "test-readonly-rpc.cjs")], "readonly-rpc-tests.log")
        stage("Fee evidence binding regressions", [sys.executable, str(VALIDATION / "test-fee-evidence.py")], "fee-evidence-tests.log")
        stage("Run evidence negative regressions", [sys.executable, str(VALIDATION / "test-rehearsal-run.py")], "rehearsal-run-tests.log")
        stage("Runtime lifecycle and rollback regressions", [node, "--test", str(VALIDATION / "test-runtime-execution.cjs")], "runtime-execution-tests.log")
        stage("SDK2609 compatibility exception regressions", [sys.executable, str(VALIDATION / "test-sdk2609-compatibility.py")], "sdk2609-compatibility-tests.log")
        stage("Authorized heartbeat fixture encoding regressions", [node, "--test", str(VALIDATION / "test-authorized-heartbeat.cjs")], "authorized-heartbeat-tests.log")
        stage("SDK source provenance negative regressions", [sys.executable, str(VALIDATION / "test-sdk-patches.py")], "sdk-patches-tests.log")
        stage("SDK primitives source reconstruction", [sys.executable, str(VALIDATION / "verify-sdk-patches.py")], "sdk-primitives-provenance.log", "sdk-primitives-provenance.json")
        stage("Async staking source provenance negative regressions", [sys.executable, str(VALIDATION / "test-staking-async-source.py"), "--upstream-repo", str(options.upstream_repo)], "staking-async-source-tests.log")
        stage("Async staking source reconstruction", [sys.executable, str(VALIDATION / "verify-staking-async-source.py"), "--upstream-repo", str(options.upstream_repo), "--output", str(VALIDATION / "staking-async-source-proof.json")], "staking-async-source-proof.log", "staking-async-source-proof.json")
        stage("Exact candidate compatibility", [node, str(VALIDATION / "verify-wasm-compatibility.cjs"), "--candidate", str(candidate), "--candidate-spec", str(options.candidate_spec), "--baseline", str(VALIDATION / "live-runtime.compact.compressed.wasm"), "--output-dir", str(VALIDATION)], "wasm-compatibility.log", "wasm-compatibility.json")
        stage("Independent complete bridge payload recovery", [node, str(VALIDATION / "verify-bridge-payload-recovery.cjs"), str(VALIDATION / "bridge-payload-evidence.json"), str(VALIDATION / "bridge-request-recovery-source.json"), str(VALIDATION / "bridge-payload-recovery-verification.json"), str(VALIDATION / "bridge-archived-payload-source.json"), str(VALIDATION / "bridge-request-recovery-capture.json")], "bridge-payload-recovery-verification.log", "bridge-payload-recovery-verification.json")
        stage("Offline candidate governance signing compatibility", [options.signing_python, str(VALIDATION / "check-python-candidate-signing.py"), "--metadata", str(VALIDATION / "candidate-metadata.scale"), "--runtime-report", str(VALIDATION / "wasm-compatibility.json"), "--out", str(VALIDATION / "python-candidate-signing-report.json")], "python-candidate-signing.log", "python-candidate-signing-report.json")
        common = ["--endpoint", snapshot_data["endpoint"], "--wasm", str(candidate), "--candidate-spec", str(options.candidate_spec), "--synthetic-fixtures", "--snapshot", str(snapshot), "--cache", str(VALIDATION / ".public-read-cache.json.gz")]
        stage("Equivocation and fee execution", [node, str(VALIDATION / "rehearse-equivocation-fees.cjs"), *common, "--output", str(VALIDATION / "equivocation-fee-wasm-rehearsal.json")], "equivocation-fee-wasm-rehearsal.log", "equivocation-fee-wasm-rehearsal.json")
        stage("Independent fee event verification", [sys.executable, str(VALIDATION / "verify-fee-events.py")], "fee-event-check.log", "fee-event-check.json")
        stage("Fee policy and bridge fixtures", [node, str(VALIDATION / "rehearse-fee-policy.cjs"), *common, "--output", str(VALIDATION / "fee-policy-wasm-rehearsal.json")], "fee-policy-wasm-rehearsal.log", "fee-policy-wasm-rehearsal.json")
        stage("Bridge readiness evidence regressions", [node, "--test", str(VALIDATION / "test-bridge-readiness-policy.cjs"), str(VALIDATION / "test-bridge-rpc-size.cjs"), str(VALIDATION / "test-bridge-payload-recovery.cjs")], "bridge-readiness-policy-tests.log")
        stage("Complete pending bridge inventory", [node, str(VALIDATION / "check-bridge-readiness.cjs")], "bridge-readiness.log", "bridge-readiness.json")
        stage("Current finalized bridge readiness", [node, str(VALIDATION / "check-bridge-readiness.cjs"), "--current"], "bridge-activation-current.log", "bridge-activation-current.json")
        stage("Independent bridge readiness verification", [node, str(VALIDATION / "verify-bridge-readiness.cjs"), "--current"], "bridge-readiness-verification.log", "bridge-readiness-verification.json")
        stage("Verify full source after rehearsal", [sys.executable, str(VALIDATION / "source-fingerprint.py"), "--check", str(fingerprint_path), "--source-only", "--output", str(VALIDATION / "source-after-rehearsal.json")], "source-after-rehearsal.log", "source-after-rehearsal.json")
        compatibility = read(VALIDATION / "wasm-compatibility.json")
        equivocation = read(VALIDATION / "equivocation-fee-wasm-rehearsal.json")
        fees = read(VALIDATION / "fee-event-check.json")
        policy = read(VALIDATION / "fee-policy-wasm-rehearsal.json")
        bridge = read(VALIDATION / "bridge-readiness.json")
        provenance = read(VALIDATION / "sdk-primitives-provenance.json")
        staking_source = read(VALIDATION / "staking-async-source-proof.json")
        signing = read(VALIDATION / "python-candidate-signing-report.json")
        assert signing["candidate_sha256"] == candidate_hash
        assert signing["runtime_report_sha256"] == digest(VALIDATION / "wasm-compatibility.json")
        assert signing["metadata_sha256"] == compatibility["inputs"]["candidate"]["metadataScaleSha256"]
        assert len(signing["cases"]) == 8 and signing["rpc_calls"] == signing["submission_calls"] == 0
        assert provenance["status"] == "passed" and len(provenance["packages"]) == 7
        assert staking_source["status"] == "passed" and staking_source["upstreamCommit"] == "7a8daa68e18718c18823c1109513fdc1c8aa71fe"
        assert policy["checks"]["fullPinnedStakingMigrationVerified"]
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
        report["asyncStakingMigration"] = {
            "reviewedCompatibility": compatibility["checks"]["reviewedAsyncStakingMigrationCompatible"],
            "storageVersions": {"from": 16, "to": 18},
            "sourceProofSha256": digest(VALIDATION / "staking-async-source-proof.json"),
            "completePinnedWitnessChecks": len(policy["asyncStakingMigration"]["checks"]),
            "capturedKeys": policy["asyncStakingMigration"]["checks"][0]["capturedKeys"],
            "fullPinnedStakingMigrationVerified": policy["checks"]["fullPinnedStakingMigrationVerified"],
        }
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
