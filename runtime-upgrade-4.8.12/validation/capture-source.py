#!/usr/bin/env python3
"""Capture and independently reapply the release source without staging user files."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

PACKAGE = Path(__file__).resolve().parents[1]
ROOT = PACKAGE.parent
PATHS = [
    "Cargo.toml", "Cargo.lock", "runtime", "common", "node/Cargo.toml",
    "node/chain_spec/Cargo.toml", "vendor/polkadot-sdk/frame/babe",
    "vendor/polkadot-sdk/frame/grandpa", "vendor/polkadot-sdk/frame/preimage",
    "vendor/pallet-multisig/src", "vendor/sora2-common/pallets",
    "vendor/sora2-common/Cargo.toml", "vendor/sora2-common/Cargo.lock",
    "pallets/apollo-platform", "pallets/eth-bridge", "pallets/iroha-migration",
    "pallets/kensetsu", "pallets/order-book", "pallets/rewards", "pallets/xor-fee",
]
REQUIRED_BUILD_INPUTS = ["Cargo.toml", "Cargo.lock", "vendor/sora2-common/Cargo.toml", "vendor/sora2-common/Cargo.lock"]
REQUIRED_FOLLOWUP_INPUTS = [
    "pallets/eth-bridge/src/lib.rs", "pallets/iroha-migration/src/lib.rs",
    "pallets/xor-fee/src/lib.rs", "runtime/src/lib.rs", "runtime/src/xor_fee_impls.rs",
    "runtime/src/tests/liveness/bridge_fees.rs", "runtime/src/tests/liveness/migration_fees.rs",
    "runtime/src/tests/liveness.rs", "pallets/iroha-migration/src/tests.rs",
]
REQUIRED_DELETIONS = ["runtime/src/migration_fees.rs", "runtime/src/tests/liveness/migration_sponsorship.rs"]
EXPECTED_BASE = "bb38216396835fae45de9c38104e4927a96eb36c"


def git(*args, env=None):
    return subprocess.check_output(["git", *args], cwd=ROOT, env=env)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    base = git("rev-parse", "HEAD").decode().strip()
    assert base == EXPECTED_BASE, "Paid-migration package must capture the reviewed bb382163 base"
    with tempfile.TemporaryDirectory(prefix="sora-4812-source-") as temporary:
        env = {**os.environ, "GIT_INDEX_FILE": str(Path(temporary) / "capture.index")}
        git("read-tree", base, env=env)
        git("add", "-A", "--", *PATHS, env=env)
        patch = git("diff", "--cached", "--binary", base, "--", *PATHS, env=env)
        tree = git("write-tree", env=env).decode().strip()
        changed = git("diff", "--cached", "--name-only", "-z", base, env=env)
        deleted_paths = git("diff", "--cached", "--name-only", "--diff-filter=D", "-z", base, env=env)
        (PACKAGE / "source.patch").write_bytes(patch)
        verify_env = {**os.environ, "GIT_INDEX_FILE": str(Path(temporary) / "verify.index")}
        git("read-tree", base, env=verify_env)
        # Retain the upstream license text byte for byte, including its blank lines.
        git("apply", "--cached", "--whitespace=nowarn", "--check", str(PACKAGE / "source.patch"), env=verify_env)
        git("apply", "--cached", "--whitespace=nowarn", str(PACKAGE / "source.patch"), env=verify_env)
        verified_tree = git("write-tree", env=verify_env).decode().strip()
        assert verified_tree == tree, "Clean-base patch replay changed the source tree"
    files = {}
    # An incremental patch may leave both lockfiles unchanged. Bind them anyway:
    # build/dependency evidence must describe this captured source, not rely on
    # the previous package's file-hash list.
    captured_paths = set(changed.decode().strip("\0").split("\0")) | set(REQUIRED_BUILD_INPUTS) | set(REQUIRED_FOLLOWUP_INPUTS)
    for path in sorted(captured_paths):
        if path and (ROOT / path).is_file():
            files[path] = digest((ROOT / path).read_bytes())
    deleted_files = {path: digest(git("show", base + ":" + path))
                     for path in deleted_paths.decode().strip("\0").split("\0") if path}
    assert set(REQUIRED_DELETIONS) <= set(deleted_files), "Sponsorship helper/test deletions must be captured"
    assert all(not (ROOT / path).exists() for path in deleted_files), "Deleted source unexpectedly exists"
    assert set(REQUIRED_FOLLOWUP_INPUTS) <= set(files), "Required paid-migration sources must exist"
    report = {
        "status": "passed", "baseCommit": base,
        "sourceTreeAfterPatch": tree, "sourceIsUncommittedOverlay": True,
        "patch": "source.patch", "patchSha256": digest(patch),
        "patchAppliesToCleanBase": True, "replayedTreeMatches": True,
        "userIndexModified": False, "preexisting4811PackageChangesIncluded": False,
        "sdkCommit": "e3737178ec726cffe506c907263aaaa417893fd0",
        "cargoTargetDirectory": "/Users/takemiyamakoto/dev/.sora2-pr1366-target",
        "requiredBuildInputs": REQUIRED_BUILD_INPUTS,
        "requiredFollowupInputs": REQUIRED_FOLLOWUP_INPUTS,
        "requiredDeletions": REQUIRED_DELETIONS,
        "deletedFiles": deleted_files,
        "buildInputScope": "Runtime compilation inputs; regenerated chain-spec blobs are bound separately in validation/chain-spec-runtimes.json.",
        "files": files,
    }
    (PACKAGE / "validation/source-provenance.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"baseCommit": base, "tree": tree, "sourceFiles": len(files), "patchReplay": "passed"}))


if __name__ == "__main__":
    main()
