#!/usr/bin/env python3
"""Build a provenance-bound SORA Nexus public fixture in its persistent Cargo lane.

This runner refuses production/unbound source, sanitizes inherited build controls,
quarantines only stale Wasm exports, and records actual exit/log/artifact hashes.
It does not publish, sign, deploy or establish production readiness.
"""
from __future__ import annotations
import argparse
import datetime
from contextlib import contextmanager
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib
from uuid import uuid4

from bootstrap.prepare import ROOT, SOURCE_PATHS
import release_package as package

ENVIRONMENT_HELPER = ROOT / "runtime-rehearsal-async-staking-2026-10-25/validation/build_environment.py"
LLVM_WRAPPER = ROOT / "scripts/with_llvm_env.sh"
EXPORTS = ("Cargo.toml", "framenode_runtime.compact.wasm", "framenode_runtime.compact.compressed.wasm")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return package.digest_file(path)


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def source_provenance(source):
    marker = source / "NEVER-DEPLOY.txt"
    require(marker.is_file() and "NEVER DEPLOY" in marker.read_text(), "Missing fixture-only NEVER-DEPLOY marker")
    candidates = [source / name for name in ("FIXTURE-PROVENANCE.json", "RECOVERY-PROVENANCE.json") if (source / name).is_file()]
    require(len(candidates) == 1, "Exactly one fixture source provenance is required")
    path = candidates[0]
    value = json.loads(path.read_text())
    require(value.get("fixtureOnly") is True, "Production or unbound development source is not a fixture build input")
    expected = (value.get("specVersion"), value.get("transactionVersion"))
    require(expected in ((134, 131), (135, 132), (136, 132), (137, 132), (138, 134)), "Unexpected fixture runtime identity")
    if expected[0] != 138:
        require(value.get("bound") is True and value.get("sdk") == "polkadot-stable2603-3", "Legacy fixture requires a bound preserved-source export")
    return path, value


def resolve_legacy_lock(source, reference, provenance):
    before = source / "Cargo.lock"
    require(provenance["specVersion"] in (134, 135, 136, 137), "Resolved lock is only for legacy fixture overlays")
    old, new = tomllib.loads(before.read_text()), tomllib.loads(reference.read_text())
    old_packages = {(item["name"], item["version"], item.get("source")): item for item in old["package"]}
    new_packages = {(item["name"], item["version"], item.get("source")): item for item in new["package"]}
    require(old_packages.keys() == new_packages.keys(), "Resolved fixture lock changes package identities")
    for key, prior in old_packages.items():
        current = new_packages[key]
        if key[0] != "framenode-runtime":
            require(prior == current, "Resolved fixture lock changes a non-runtime package")
            continue
        old_deps, new_deps = set(prior.get("dependencies", [])), set(current.get("dependencies", []))
        require(old_deps <= new_deps and new_deps - old_deps <= {"sp-trie", "sp-externalities"}, "Unreviewed runtime lock dependency change")
        require({k: v for k, v in prior.items() if k != "dependencies"} == {k: v for k, v in current.items() if k != "dependencies"}, "Resolved runtime package changed")
    require({k: v for k, v in old.items() if k != "package"} == {k: v for k, v in new.items() if k != "package"}, "Resolved lock format changed")
    evidence = {"original": digest(before), "resolved": digest(reference), "allowedDirectDependencies": ["sp-trie", "sp-externalities"], "packageIdentityDrift": False}
    if before.read_bytes() != reference.read_bytes():
        before.write_bytes(reference.read_bytes())
    return evidence


def fingerprint(source, provenance, allow_lock=None):
    if provenance["specVersion"] == 138:
        expected = dict(provenance["sourceFiles"])
        expected["runtime/src/incident_recovery/generated.rs"] = provenance["generatedPolicy"]
    else:
        expected = dict(provenance["overlay"])
    if allow_lock:
        expected["Cargo.lock"] = allow_lock
    for relative, binding in expected.items():
        path = package.local_file(source, relative)
        actual = digest(path)
        require(all(actual[name] == binding[name] for name in ("sha256", "blake2_256")), "Fixture source differs from export provenance: " + relative)
    files = {}
    for entry in SOURCE_PATHS:
        path = source / entry
        candidates = path.rglob("*") if path.is_dir() else [path]
        for item in candidates:
            if item.is_file():
                relative = item.relative_to(source)
                require(not item.is_symlink(), "Fixture build source contains a symlink")
                if not any(part in ("target", "node_modules", ".git") for part in relative.parts):
                    files[relative.as_posix()] = digest(item)
    require("Cargo.lock" in files and "runtime/src/incident_recovery/generated.rs" in files, "Incomplete fixture source fingerprint")
    return dict(sorted(files.items()))


def build_environment(source, run_id, target=None):
    source = source.resolve(strict=True)
    spec = importlib.util.spec_from_file_location("soranx_fixture_build_environment", ENVIRONMENT_HELPER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    env, policy = module.prepare_build_environment(os.environ, source, run_token=run_id)
    extra = {"WASM_BUILD_WORKSPACE_HINT": str(source), "WASM_BUILD_CARGO_ARGS": "--offline",
             "CC_wasm32v1_none": "/opt/homebrew/opt/llvm@22/bin/clang", "AR_wasm32v1_none": "/opt/homebrew/opt/llvm@22/bin/llvm-ar",
             "CC_wasm32_unknown_unknown": "/opt/homebrew/opt/llvm@22/bin/clang", "AR_wasm32_unknown_unknown": "/opt/homebrew/opt/llvm@22/bin/llvm-ar"}
    policy["removedKeys"] = sorted(set(policy["removedKeys"]) | {name for name in extra if name in env})
    # The parent command and WASM_BUILD_CARGO_ARGS each explicitly use --offline.
    # The SDK also translates CARGO_NET_OFFLINE=true into --offline, which would
    # duplicate the nested Cargo argument. Retain one explicit mechanism per call.
    for name in ("DYLD_LIBRARY_PATH", "CARGO_NET_OFFLINE"):
        if name in env:
            env.pop(name)
            policy["removedKeys"] = sorted(set(policy["removedKeys"]) | {name})
    env.update(extra)
    policy["controlled"].update(extra)
    if target is not None:
        target = target.resolve(strict=True)
        env["CARGO_TARGET_DIR"] = str(target)
        policy["controlled"]["CARGO_TARGET_DIR"] = str(target)
    return env, policy


def quarantine(project, output):
    archive = output / "previous-exports"
    archive.mkdir()
    prior = []
    for name in EXPORTS:
        path = project / name
        require(not path.is_symlink(), "Wasm export symlink")
        if path.exists():
            require(path.is_file(), "Wasm export must be a file")
            prior.append({"file": name, **digest(path)})
            path.rename(archive / name)
    require(all(not (project / name).exists() for name in EXPORTS), "Stale Wasm exports remain")
    return prior


def validate_wasm_project(project, source):
    manifest = tomllib.loads((project / "Cargo.toml").read_text())
    dependency = manifest["dependencies"]["wasm-project"]
    require(dependency.get("default-features") is False and dependency.get("features") == ["build-wasm-binary"], "Unexpected fixture Wasm features")
    require(dependency.get("package") == "framenode-runtime" and isinstance(dependency.get("path"), str), "Unexpected fixture Wasm package")
    runtime_path = Path(dependency["path"])
    if not runtime_path.is_absolute():
        runtime_path = project / runtime_path
    require(runtime_path.resolve(strict=True) == (source / "runtime").resolve(strict=True), "Wasm project references a different fixture source")
    return dependency


@contextmanager
def operation_lock(path, error_message):
    with path.open("a+") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError(error_message) from error
        try:
            yield
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


def source_lock(source):
    return operation_lock(source.resolve(strict=True) / ".fixture-operation.lock", "Another fixture build or refresh is using this source lane")


def target_lock(target):
    return operation_lock(target.resolve(strict=True) / ".fixture-build.lock", "Another fixture build is using this Cargo target lane")


def build(source, output, resolved_lock=None, target_dir=None):
    source = source.resolve(strict=True)
    target = (target_dir or source / "target").resolve()
    require(target != source and target not in source.parents, "Cargo target cannot replace fixture source or an ancestor")
    target.mkdir(parents=True, exist_ok=True)
    target = target.resolve(strict=True)
    with source_lock(source):
        with target_lock(target):
            return build_locked(source, output, resolved_lock, target)


def build_locked(source, output, resolved_lock, target):
    provenance_path, provenance = source_provenance(source)
    require(not output.exists(), "Build evidence output must be new; Cargo target is reused separately")
    output.mkdir(parents=True)
    report = {"schema": "sora-nexus-fixture-wasm-build-v1", "fixtureOnly": True, "productionReady": False,
              "status": "running", "startedAt": now(), "source": str(source), "specVersion": provenance["specVersion"],
              "transactionVersion": provenance["transactionVersion"], "sourceProvenance": digest(provenance_path),
              "cargoTargetDirectory": str(target), "targetLock": str(target / ".fixture-build.lock"),
              "runner": digest(Path(__file__)), "environmentHelper": digest(ENVIRONMENT_HELPER), "llvmWrapper": digest(LLVM_WRAPPER)}
    evidence_path = output / "build.json"
    def save():
        temporary = output / "build.json.tmp"
        temporary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        temporary.replace(evidence_path)
    save()
    for path, name in ((Path(__file__), "build-fixture-runner.py"), (ENVIRONMENT_HELPER, "build-environment.py"), (LLVM_WRAPPER, "llvm-wrapper.sh")):
        shutil.copy2(path, output / name)
    try:
        if resolved_lock:
            report["legacyLockAmendment"] = resolve_legacy_lock(source, resolved_lock.resolve(strict=True), provenance)
        lock_before = digest(source / "Cargo.lock")
        allow_lock = report.get("legacyLockAmendment", {}).get("resolved")
        before = fingerprint(source, provenance, allow_lock)
        (output / "source-files.json").write_text(json.dumps(before, indent=2, sort_keys=True) + "\n")
        report["sourceFingerprint"] = digest(output / "source-files.json")
        report["lockBefore"] = lock_before
        run_id = uuid4().hex
        env, policy = build_environment(source, run_id, target)
        report["environmentPolicy"] = policy
        report["runId"] = run_id
        report["rustc"] = subprocess.check_output(["rustc", "-Vv"], cwd=source, env=env, text=True)
        project = target / "debug/wbuild/framenode-runtime"
        report["previousExports"] = quarantine(project, output)
        command = [str(LLVM_WRAPPER), "cargo", "check", "--offline", "--locked", "-p", "framenode-runtime", "--lib", "--features", "build-wasm-binary"]
        report["command"] = command
        save()
        with (output / "cargo.stdout.log").open("wb") as stdout, (output / "cargo.stderr.log").open("wb") as stderr:
            result = subprocess.run(command, cwd=source, env=env, stdout=stdout, stderr=stderr)
        report["exitCode"] = result.returncode
        report["logs"] = {name: digest(output / name) for name in ("cargo.stdout.log", "cargo.stderr.log")}
        report["lockAfter"] = digest(source / "Cargo.lock")
        require(report["lockAfter"] == lock_before, "Locked fixture build changed Cargo.lock")
        require(fingerprint(source, provenance, allow_lock) == before, "Fixture source changed during build")
        require(digest(provenance_path) == report["sourceProvenance"], "Fixture provenance changed during build")
        require(result.returncode == 0, f"Cargo fixture build failed with exit {result.returncode}")
        for name in EXPORTS:
            path = project / name
            require(path.is_file() and not path.is_symlink() and path.stat().st_size > 0, "Build did not recreate " + name)
        dependency = validate_wasm_project(project, source)
        compressed = project / EXPORTS[2]
        identity = package.wasm_identity(compressed.read_bytes())
        require((identity["specVersion"], identity["transactionVersion"]) == (provenance["specVersion"], provenance["transactionVersion"]), "Built fixture identity mismatch")
        report["runtimeIdentity"] = identity
        report["wasmProjectDependency"] = dependency
        for source_path, name in ((project / "Cargo.toml", "wasm-project.toml"), (source / "Cargo.lock", "Cargo.lock"),
                                  (source / "rust-toolchain.toml", "rust-toolchain.toml"), (provenance_path, "source-provenance.json")):
            shutil.copy2(source_path, output / name)
        report["artifacts"] = {}
        for name in EXPORTS[1:]:
            output_name = name.replace("framenode_runtime", f"runtime-{provenance['specVersion']}")
            shutil.copy2(project / name, output / output_name)
            report["artifacts"][output_name] = digest(output / output_name)
        report["status"] = "passed"
    except BaseException as error:
        report.update(status="failed", error=str(error))
        raise
    finally:
        report["finishedAt"] = now()
        save()
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--resolved-legacy-lock", type=Path)
    parser.add_argument("--target-dir", type=Path, help="Explicit persistent Cargo target for sequential fixtures; exclusive target lock prevents overlap")
    args = parser.parse_args()
    report = build(args.source, args.output.resolve(), args.resolved_legacy_lock, args.target_dir)
    print(json.dumps({"status": report["status"], "output": str(args.output.resolve()), "artifacts": report["artifacts"], "fixtureOnly": True, "productionReady": False}))


if __name__ == "__main__":
    main()
