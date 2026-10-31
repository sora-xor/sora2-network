#!/usr/bin/env python3
"""Stage a complete npm graph without modifying source release evidence."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil


HISTORICAL_PREFIX = "../../private/tmp/sora-payout-runtime-hunt/"
DEPENDENCY_FIELDS = ("dependencies", "devDependencies", "optionalDependencies")


def canonical_lock(lock):
    """Relocate only the recorded historical installation, without changing packages."""
    if lock.get("lockfileVersion") != 3 or not isinstance(lock.get("packages"), dict):
        raise ValueError("expected a complete npm v3 packages graph")
    packages = lock["packages"]
    locations = [name for name in packages if name]
    if not locations or "" not in packages:
        raise ValueError("refusing an empty or rootless dependency graph")
    if any(value.get("link") for value in packages.values()):
        raise ValueError("local packages must be installed with install-links=true")
    historical = [name for name in locations if name.startswith(HISTORICAL_PREFIX)]
    if historical and len(historical) == len(locations):
        relocated = {name.removeprefix(HISTORICAL_PREFIX): value for name, value in packages.items()}
        if len(relocated) != len(packages):
            raise ValueError("historical path relocation would collide")
        return {**lock, "packages": relocated}, len(historical)
    for name in locations:
        if name.startswith("node_modules/"):
            continue
        if name.startswith(HISTORICAL_PREFIX) and packages[name].get("extraneous") is True:
            continue
        raise ValueError(f"unrecognized package location: {name}")
    return lock, 0


REVIEWED_PACKAGES = {
    "@polkadot/wasm-crypto": "vendor/polkadot-wasm-crypto-js",
    "@polkadot/wasm-crypto-wasm": "vendor/polkadot-wasm-crypto-wasm",
    "@polkadot/rpc-provider": "vendor/polkadot-rpc-provider",
    "fetch-blob": "vendor/fetch-blob",
    "@acala-network/chopsticks-core": "vendor/chopsticks-core",
    "@acala-network/chopsticks-executor": "vendor/chopsticks-executor",
}


OPERATIONAL_RUNTIME_PROJECTS = {
    f"runtime-upgrade-{version}/validation"
    for version in ("4.8.9", "4.8.10", "4.8.11", "4.8.12")
} | {"runtime-rehearsal-dependency-migrations-2026-10-25/validation",
     "runtime-rehearsal-sdk2609-2026-10-25/validation",
     "runtime-rehearsal-async-staking-2026-10-25/validation"}
OPERATIONAL_PROJECTS = OPERATIONAL_RUNTIME_PROJECTS | {
    "misc/polkadotjs/bug-hunt/frontend-repro"
}


def check_reviewed_native_packages(lock, repo, project):
    """Require the reviewed endpoints even after a direct pin is changed/removed."""
    packages = lock["packages"]
    direct = packages[""].get("dependencies", {})
    repo, project = Path(repo).resolve(), Path(project).resolve()
    project_name = project.relative_to(repo).as_posix()

    def records(name):
        return [data for location, data in packages.items()
                if location.endswith("node_modules/" + name)]

    # Do not let removing the pin disable the policy while transitive native
    # packages remain. Generic unrelated graphs may still use this staging tool.
    if project_name not in OPERATIONAL_PROJECTS and not any(
            name in direct or records(name) for name in REVIEWED_PACKAGES):
        return
    required = ["@polkadot/wasm-crypto", "@polkadot/wasm-crypto-wasm",
                "@polkadot/rpc-provider", "fetch-blob"]
    direct_pins = ["@polkadot/wasm-crypto", "@polkadot/rpc-provider", "fetch-blob"]
    chopsticks = ["@acala-network/chopsticks-core", "@acala-network/chopsticks-executor"]
    if project_name in OPERATIONAL_RUNTIME_PROJECTS or any(
            name in direct or records(name) for name in chopsticks):
        required.extend(chopsticks)
        direct_pins.append("@acala-network/chopsticks-core")
    retired = ("@substrate/connect", "smoldot", "@polkadot/wasm-crypto-asmjs",
               "@polkadot/wasm-crypto-init", "@polkadot/wasm-bridge", "node-domexception")
    for name in retired:
        if records(name):
            raise ValueError(f"retired embedded native package reintroduced: {name}")

    def expected_endpoint(name, spec):
        return (isinstance(spec, str) and spec.startswith("file:")
                and (project / spec[5:]).resolve() == repo / REVIEWED_PACKAGES[name])

    for name in direct_pins:
        if not expected_endpoint(name, direct.get(name)):
            raise ValueError(f"required reviewed direct pin missing or changed: {name}")
    for name in required:
        installed = records(name)
        if len(installed) != 1 or not expected_endpoint(name, installed[0].get("resolved")):
            raise ValueError(f"expected exactly one canonical reviewed local package: {name}")


def prepare(repo, manifest, lockfile, destination):
    repo = Path(repo).resolve()
    manifest, lockfile, destination = (Path(p).resolve() for p in (manifest, lockfile, destination))
    manifest_relative = manifest.relative_to(repo)
    lockfile.relative_to(repo)
    if manifest.parent != lockfile.parent:
        raise ValueError("manifest and lockfile must belong to the same directory")
    if destination == repo or destination.is_relative_to(repo):
        raise ValueError("audit destination must be outside the repository")
    destination.mkdir(parents=True, exist_ok=True)
    if any(destination.iterdir()):
        raise ValueError("audit destination must be empty")
    manifest_bytes, lock_bytes = manifest.read_bytes(), lockfile.read_bytes()
    package = json.loads(manifest_bytes)
    lock, relocated = canonical_lock(json.loads(lock_bytes))
    for field in DEPENDENCY_FIELDS:
        if package.get(field, {}) != lock["packages"][""].get(field, {}):
            raise ValueError(f"manifest and lockfile disagree on {field}")
    check_reviewed_native_packages(lock, repo, manifest.parent)
    local_sources = set()

    def copy_local_dependencies(data, owner):
        for field in DEPENDENCY_FIELDS:
            for spec in data.get(field, {}).values():
                if not isinstance(spec, str) or not spec.startswith("file:"):
                    continue
                source = (owner / spec[5:]).resolve()
                relative = source.relative_to(repo)
                if not source.is_dir() or not (source / "package.json").is_file():
                    raise ValueError(f"local dependency is not a package directory: {source}")
                if source in local_sources:
                    continue
                local_sources.add(source)
                shutil.copytree(source, destination / relative, ignore=shutil.ignore_patterns(
                    "node_modules", "target", ".git", "__pycache__"
                ))
                copy_local_dependencies(json.loads((source / "package.json").read_text()), source)

    copy_local_dependencies(package, manifest.parent)
    staged = destination / manifest_relative.parent
    staged.mkdir(parents=True, exist_ok=True)
    (staged / "package.json").write_bytes(manifest_bytes)
    (staged / "package-lock.json").write_text(json.dumps(lock, indent=2) + "\n")
    (staged / ".npmrc").write_text("install-links=true\nignore-scripts=true\n")
    evidence = {
        "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
        "lockfile_sha256": hashlib.sha256(lock_bytes).hexdigest(),
        "package_records": len(lock["packages"]) - 1,
        "relocated_records": relocated,
        "local_packages": sorted(str(p.relative_to(repo)) for p in local_sources),
    }
    (destination / "source-evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
    assert manifest.read_bytes() == manifest_bytes and lockfile.read_bytes() == lock_bytes
    return staged


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("lockfile", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    print(prepare(Path(__file__).resolve().parents[2], args.manifest, args.lockfile, args.destination))
