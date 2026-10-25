#!/usr/bin/env python3
"""Read-only verification of the exact SDK async staking fork and manifest policy.

Example:
  python3 verify-staking-async-source.py --upstream-repo /path/to/polkadot-sdk
The upstream checkout's working files are never read. Git archive supplies both
manifests and all source bytes from the immutable commit named below.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import tomllib

COMMIT = "7a8daa68e18718c18823c1109513fdc1c8aa71fe"
TAG = "polkadot-stable2609"
SDK_URL = "https://github.com/paritytech/polkadot-sdk.git"
SDK_PATH = Path("substrate/frame/staking-async")
VENDOR_PATH = Path("vendor/polkadot-sdk/frame/staking-async")
# These upstream sibling packages are dependencies/runtimes, not the vendored pallet.
EXCLUDED_SUBCRATES = frozenset({"ah-client", "rc-client", "runtimes"})


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def run(command: list[str], *, cwd: Path | None = None) -> bytes:
    result = subprocess.run(command, cwd=cwd, capture_output=True, check=False)
    require(result.returncode == 0,
            f"Command failed: {command!r}: {result.stderr.decode(errors='replace')}")
    return result.stdout


def materialize(package: dict, workspace: dict) -> tuple[dict, list[dict]]:
    """Independent normalization: only inheritance and local SDK paths may change."""
    manifest = copy.deepcopy(package)
    changes = []
    for key, value in list(manifest["package"].items()):
        if isinstance(value, dict) and value.get("workspace") is True:
            require(set(value) == {"workspace"}, f"Unexpected package inheritance: {key}")
            manifest["package"][key] = copy.deepcopy(workspace["package"][key])
            changes.append({"kind": "packageInheritance", "field": key})
    if manifest.get("lints", {}).get("workspace"):
        require(manifest["lints"] == {"workspace": True}, "Unexpected lint inheritance")
        manifest["lints"] = copy.deepcopy(workspace.get("lints", {}))
        changes.append({"kind": "lintInheritance"})

    def normalize_dependencies(parent: dict, location: str) -> None:
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, spec in list(parent.get(section, {}).items()):
                if not isinstance(spec, dict):
                    continue
                spec = copy.deepcopy(spec)
                if spec.pop("workspace", False):
                    inherited = copy.deepcopy(workspace["dependencies"][name])
                    if isinstance(inherited, str):
                        inherited = {"version": inherited}
                    if "features" in spec:
                        spec["features"] = list(dict.fromkeys(
                            inherited.get("features", []) + spec["features"]))
                    inherited.update(spec)
                    spec = inherited
                    changes.append({"kind": "dependencyInheritance", "section": location + section,
                                    "dependency": name})
                if "path" in spec:
                    upstream_path = spec.pop("path")
                    spec.pop("version", None)
                    spec["git"] = SDK_URL
                    spec["tag"] = TAG
                    changes.append({"kind": "sdkPathToPinnedGit", "section": location + section,
                                    "dependency": name, "upstreamPath": upstream_path,
                                    "git": SDK_URL, "tag": TAG})
                parent[section][name] = spec

    normalize_dependencies(manifest, "")
    for target, parent in manifest.get("target", {}).items():
        normalize_dependencies(parent, f"target.{target}.")
    return manifest, changes


def inventory(root: Path, *, exclude_subcrates: bool = False) -> dict[str, str]:
    result = {}
    # A directory symlink could make Rust read source outside this recorded inventory.
    for path in root.rglob("*"):
        require(not path.is_symlink(), f"Symlink outside the source inventory: {path.relative_to(root)}")
    for path in sorted(root.rglob("*.rs")):
        relative = path.relative_to(root)
        if exclude_subcrates and relative.parts[0] in EXCLUDED_SUBCRATES:
            continue
        require(not path.is_symlink(), f"Rust source is a symlink: {relative}")
        result[relative.as_posix()] = sha256(path.read_bytes())
    return result


def inventory_hash(files: dict[str, str]) -> str:
    payload = "".join(f"{path}\0{digest}\n" for path, digest in sorted(files.items()))
    return sha256(payload.encode())


def verify(args: argparse.Namespace) -> dict:
    root = args.repo_root.resolve()
    vendor = root / VENDOR_PATH
    upstream_repo = args.upstream_repo.resolve()
    provenance_bytes = (vendor / "stable2609-customizations.json").read_bytes()
    provenance = json.loads(provenance_bytes)
    require(provenance.get("upstreamCommit") == COMMIT, "Provenance commit mismatch")
    require(provenance.get("upstreamTag") == TAG, "Provenance tag mismatch")
    require(provenance.get("upstreamPath") == SDK_PATH.as_posix(), "Provenance package path mismatch")
    require(provenance.get("sourcePatch") == "stable2609-customizations.patch", "Unexpected patch path")
    patch_path = vendor / provenance["sourcePatch"]
    patch = patch_path.read_bytes()
    require(sha256(patch) == provenance["sourcePatchSha256"], "Patch hash mismatch")
    original_manifest = (vendor / "Cargo.toml.orig").read_bytes()
    materialized_manifest = (vendor / "Cargo.toml").read_bytes()
    require(sha256(original_manifest) == provenance["upstreamManifestSha256"],
            "Original manifest hash mismatch")
    require(sha256(materialized_manifest) == provenance["materializedManifestSha256"],
            "Materialized manifest hash mismatch")

    actual_commit = run(["git", "rev-parse", f"{COMMIT}^{{commit}}"], cwd=upstream_repo).decode().strip()
    require(actual_commit == COMMIT, "Exact upstream commit is unavailable")
    tag_commit = run(["git", "rev-parse", f"refs/tags/{TAG}^{{commit}}"], cwd=upstream_repo).decode().strip()
    require(tag_commit == COMMIT, "Release tag no longer identifies the required exact commit")
    archive = run(["git", "archive", "--format=tar", COMMIT, "Cargo.toml", ".rustfmt.toml", str(SDK_PATH)],
                  cwd=upstream_repo)
    with tempfile.TemporaryDirectory(prefix="verify-staking-async-") as temporary:
        work = Path(temporary)
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as tar:
            # Git archive is immutable evidence, but still do not extract arbitrary links/paths.
            for member in tar.getmembers():
                relative = Path(member.name)
                require(not relative.is_absolute() and ".." not in relative.parts,
                        "Unsafe path in upstream archive")
                require(member.isfile() or member.isdir(), "Non-file entry in upstream archive")
                destination = work / relative
                if member.isdir():
                    destination.mkdir(parents=True, exist_ok=True)
                else:
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    stream = tar.extractfile(member)
                    require(stream is not None, f"Missing archive bytes: {member.name}")
                    destination.write_bytes(stream.read())
        archived = work / SDK_PATH
        archived_manifest = (archived / "Cargo.toml").read_bytes()
        require(archived_manifest == original_manifest,
                "Cargo.toml.orig differs from exact upstream Git object")
        expected_manifest, adaptations = materialize(
            tomllib.loads(archived_manifest.decode()),
            tomllib.loads((work / "Cargo.toml").read_text())["workspace"],
        )
        require(tomllib.loads(materialized_manifest.decode()) == expected_manifest,
                "Local Cargo.toml contains adaptations beyond explicit inheritance/path policy")
        format_config = (vendor / ".rustfmt.toml").read_bytes()
        require(format_config == (work / ".rustfmt.toml").read_bytes(),
                "Formatter configuration differs from exact upstream SDK root")
        require(sha256(format_config) == provenance["rustfmtConfigSha256"],
                "Formatter configuration hash mismatch")
        upstream_files = inventory(archived, exclude_subcrates=True)
        patch_in_temp = work / "recorded.patch"
        patch_in_temp.write_bytes(patch)
        # The recorded source patch cannot hide unrelated manifest/config/file changes.
        stats = run(["git", "apply", "--numstat", "-z", str(patch_in_temp)], cwd=archived)
        for entry in filter(None, stats.split(b"\0")):
            fields = entry.split(b"\t", 2)
            require(len(fields) == 3, "Unexpected source patch filename format")
            relative = Path(fields[2].decode())
            require(relative.suffix == ".rs" and not relative.is_absolute()
                    and ".." not in relative.parts and relative.parts[0] not in EXCLUDED_SUBCRATES,
                    f"Source patch touches a file outside the Rust inventory: {relative}")
        # No --unsafe-paths: a patch cannot write outside the temporary archived package.
        run(["git", "apply", "--check", "--whitespace=nowarn", str(patch_in_temp)], cwd=archived)
        run(["git", "apply", "--whitespace=nowarn", str(patch_in_temp)], cwd=archived)
        replay_files = inventory(archived, exclude_subcrates=True)
        local_files = inventory(vendor)
        require(replay_files == local_files,
                "Complete Rust source inventory differs after patch replay: " + ", ".join(
                    path for path in sorted(set(replay_files) | set(local_files))
                    if replay_files.get(path) != local_files.get(path)))
        changes = [{"path": path, "upstreamSha256": upstream_files.get(path),
                    "localSha256": local_files.get(path)}
                   for path in sorted(set(upstream_files) | set(local_files))
                   if upstream_files.get(path) != local_files.get(path)]
        require(changes == provenance["changedSourceFiles"], "Recorded source-change inventory mismatch")
        return {"status": "passed", "verifierSha256": sha256(Path(__file__).read_bytes()),
                "upstreamCommit": COMMIT, "upstreamTag": TAG,
                "upstreamGitTree": run(["git", "rev-parse", f"{COMMIT}^{{tree}}"], cwd=upstream_repo).decode().strip(),
                "scope": {"package": SDK_PATH.as_posix(), "excludedSiblingCrates": sorted(EXCLUDED_SUBCRATES),
                          "includesUnchangedRustFiles": True},
                "upstreamArchiveSha256": sha256(archive),
                "provenanceSha256": sha256(provenance_bytes), "sourcePatchSha256": sha256(patch),
                "upstreamManifestSha256": sha256(original_manifest),
                "materializedManifestSha256": sha256(materialized_manifest),
                "rustfmtConfigSha256": sha256(format_config),
                "normalizedManifestSha256": sha256(json.dumps(expected_manifest, sort_keys=True, separators=(",", ":")).encode()),
                "manifestAdaptations": adaptations,
                "upstreamRustFileCount": len(upstream_files), "localRustFileCount": len(local_files),
                "changedRustFileCount": len(changes),
                "upstreamRustInventorySha256": inventory_hash(upstream_files),
                "localRustInventorySha256": inventory_hash(local_files),
                "rustFiles": local_files,
                "verification": {"originalManifestMatchesGitObject": True,
                                 "manifestAdaptationsExplicitlyVerified": True,
                                 "sourcePatchReplay": "passed", "completeRustInventoryMatches": True}}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream-repo", required=True, type=Path,
                        help="Local Git object database containing the exact SDK release")
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", type=Path, help="Optional JSON evidence output (otherwise stdout)")
    args = parser.parse_args()
    try:
        result = verify(args)
        exit_code = 0
    except (ValueError, OSError, KeyError, TypeError, tarfile.TarError) as error:
        result = {"status": "failed", "error": str(error)}
        exit_code = 1
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(encoded)
    else:
        sys.stdout.write(encoded)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
