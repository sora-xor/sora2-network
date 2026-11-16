#!/usr/bin/env python3
"""Assemble the operator restart kit: launcher, Dockerfile, chain spec and work bundle.

The kit is everything a validator needs beside the snapshot, for both the Docker
and the from-source route. `operator_kit/sora-recovery` pins the source commit,
snapshot, chain spec and bundle; this refuses inputs that differ from those pins,
so the kit, the image built from it and the release sheet name the same release.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
KIT_FILES = ("sora-recovery", "Dockerfile", ".dockerignore", "README.md")
SPEC_NAME = "sora-nexus-recovery-28191817.json"
BUNDLE_NAME = "sora-nexus-recovery-work-bundle"


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pins(launcher: Path) -> dict:
    text = launcher.read_text()
    return {name: re.search(rf"^{name}=(\S+)$", text, re.M).group(1)
            for name in ("SOURCE_COMMIT", "SNAPSHOT_SHA256", "SPEC_SHA256", "BUNDLE_SHA256")}


def prepare(spec: Path, bundle: Path, destination: Path) -> dict:
    source = HERE / "operator_kit"
    pin = pins(source / "sora-recovery")
    if sha(spec) != pin["SPEC_SHA256"]:
        raise SystemExit(f"{spec} is not the chain spec pinned in sora-recovery")
    if sha(bundle / "BUNDLE.json") != pin["BUNDLE_SHA256"]:
        raise SystemExit(f"{bundle} is not the work bundle pinned in sora-recovery")
    destination.mkdir(parents=False)
    for name in KIT_FILES:
        shutil.copyfile(source / name, destination / name)
    (destination / "sora-recovery").chmod(0o755)
    shutil.copyfile(spec, destination / SPEC_NAME)
    work = destination / BUNDLE_NAME
    work.mkdir()
    meta = json.loads((bundle / "BUNDLE.json").read_bytes())
    for row in meta["envelopes"]:
        shutil.copyfile(bundle / row["file"], work / row["file"])
        if sha(work / row["file"]) != row["sha256"]:
            raise SystemExit(f"bundle file differs from BUNDLE.json: {row['file']}")
    shutil.copyfile(bundle / "BUNDLE.json", work / "BUNDLE.json")
    shutil.copyfile(HERE / "recovery_feeder.py", work / "recovery_feeder.py")
    files = sorted(path for path in destination.rglob("*") if path.is_file())
    (destination / "SHA256SUMS").write_text(
        "".join(f"{sha(path)}  {path.relative_to(destination).as_posix()}\n" for path in files))
    archive = destination.parent / f"{destination.name}.tar.gz"
    tar = ["tar", "-C", str(destination.parent), "-czf", str(archive)]
    if "bsdtar" in subprocess.run(["tar", "--version"], capture_output=True, text=True).stdout:
        tar += ["--no-xattrs", "--no-mac-metadata", "--uid", "0", "--gid", "0", "--uname", "", "--gname", ""]
    else:
        tar += ["--owner=0", "--group=0", "--numeric-owner"]
    subprocess.run(tar + [destination.name], check=True, env={**os.environ, "COPYFILE_DISABLE": "1"})
    return {"kit": str(destination), "archive": str(archive), "archiveBytes": archive.stat().st_size,
            "archiveSha256": sha(archive), "sha256sums": sha(destination / "SHA256SUMS"),
            "launcherSha256": sha(destination / "sora-recovery"), "dockerfileSha256": sha(destination / "Dockerfile"),
            "feederSha256": sha(work / "recovery_feeder.py"), "envelopes": len(meta["envelopes"]), **pin}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", type=Path, required=True, help="release chain spec with bootnodes")
    parser.add_argument("--bundle", type=Path, required=True, help="prepare_recovery_work_bundle.py destination")
    parser.add_argument("--destination", type=Path, required=True,
                        help="new directory, conventionally sora-nexus-recovery-28191817-kit")
    args = parser.parse_args()
    json.dump(prepare(args.spec, args.bundle, args.destination), sys.stdout, indent=2)
    print()
