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


def git(*args, env=None):
    return subprocess.check_output(["git", *args], cwd=ROOT, env=env)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    base = git("rev-parse", "HEAD").decode().strip()
    with tempfile.TemporaryDirectory(prefix="sora-4812-source-") as temporary:
        env = {**os.environ, "GIT_INDEX_FILE": str(Path(temporary) / "capture.index")}
        git("read-tree", base, env=env)
        git("add", "-A", "--", *PATHS, env=env)
        patch = git("diff", "--cached", "--binary", base, "--", *PATHS, env=env)
        tree = git("write-tree", env=env).decode().strip()
        changed = git("diff", "--cached", "--name-only", "-z", base, env=env)
        (PACKAGE / "source.patch").write_bytes(patch)
        verify_env = {**os.environ, "GIT_INDEX_FILE": str(Path(temporary) / "verify.index")}
        git("read-tree", base, env=verify_env)
        # Retain the upstream license text byte for byte, including its blank lines.
        git("apply", "--cached", "--whitespace=nowarn", "--check", str(PACKAGE / "source.patch"), env=verify_env)
        git("apply", "--cached", "--whitespace=nowarn", str(PACKAGE / "source.patch"), env=verify_env)
        verified_tree = git("write-tree", env=verify_env).decode().strip()
        assert verified_tree == tree, "Clean-base patch replay changed the source tree"
    files = {}
    for path in changed.decode().strip("\0").split("\0"):
        if path and (ROOT / path).is_file():
            files[path] = digest((ROOT / path).read_bytes())
    report = {
        "status": "passed", "baseCommit": base,
        "sourceTreeAfterPatch": tree, "sourceIsUncommittedOverlay": True,
        "patch": "source.patch", "patchSha256": digest(patch),
        "patchAppliesToCleanBase": True, "replayedTreeMatches": True,
        "userIndexModified": False, "preexisting4811PackageChangesIncluded": False,
        "sdkCommit": "e3737178ec726cffe506c907263aaaa417893fd0",
        "cargoTargetDirectory": "/Users/takemiyamakoto/dev/.sora2-pr1366-target",
        "files": files,
    }
    (PACKAGE / "validation/source-provenance.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"baseCommit": base, "tree": tree, "sourceFiles": len(files), "patchReplay": "passed"}))


if __name__ == "__main__":
    main()
