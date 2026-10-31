#!/usr/bin/env python3
"""Refresh only an unchanged final-runtime fixture source, retaining its warm target.

The previous complete source and provenance are archived first. Production source,
legacy bound substitutes, modified fixture files and concurrent builds are refused.
"""
from __future__ import annotations
import argparse
import gzip
import json
from pathlib import Path
import shutil
import tarfile
import tempfile

import build_fixture_wasm as build
import fixture_artifacts as fixture


def refresh(source, policy_path, evidence, source_root=fixture.ROOT):
    source = source.resolve(strict=True)
    build.require(not evidence.exists(), "Refresh evidence destination must be new")
    with build.source_lock(source):
        provenance_path, prior = build.source_provenance(source)
        build.require(prior["specVersion"] == 138 and provenance_path.name == "FIXTURE-PROVENANCE.json" and prior.get("schema") == "sora-nexus-final-fixture-source-v1", "Only final-runtime public fixture source can be refreshed")
        policy_digest = build.digest(source / "fixture-policy.scale")
        build.require(all(policy_digest[key] == prior["policy"][key] for key in ("sha256", "blake2_256")), "Fixture policy sidecar changed")
        before = build.fingerprint(source, prior)
        build.require(set(before) == set(prior["sourceFiles"]), "Unexpected files in fixture source; refusing refresh")
        with tempfile.TemporaryDirectory(prefix="soranx-final-fixture-refresh-") as temporary:
            fresh = Path(temporary) / "source"
            following = fixture.export_final(policy_path, fresh, source_root)
            # Prepare every new byte and validate old bytes before mutation. The
            # separate target directory remains untouched across this change.
            evidence.mkdir(parents=True)
            archive_path = evidence / "previous-source.tar.gz"
            names = sorted(set(before) | {"FIXTURE-PROVENANCE.json", "NEVER-DEPLOY.txt", "fixture-policy.scale"})
            with archive_path.open("xb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed, tarfile.open(fileobj=compressed, mode="w|") as archive:
                for name in names:
                    path = source / name
                    info = tarfile.TarInfo(name)
                    info.size = path.stat().st_size
                    info.mode = path.stat().st_mode & 0o777
                    info.uid = info.gid = info.mtime = 0
                    info.uname = info.gname = ""
                    with path.open("rb") as content:
                        archive.addfile(info, content)
            build.require(build.fingerprint(source, prior) == before, "Fixture changed while archiving previous source")
            shutil.copy2(provenance_path, evidence / "previous-provenance.json")
            shutil.copy2(fresh / "FIXTURE-PROVENANCE.json", evidence / "next-provenance.json")
            report = {"schema": "sora-nexus-final-fixture-refresh-v1", "fixtureOnly": True, "productionReady": False,
                      "source": str(source), "warmTargetPreserved": True, "previousSourceArchive": build.digest(archive_path),
                      "previousProvenance": build.digest(evidence / "previous-provenance.json"),
                      "nextProvenance": build.digest(evidence / "next-provenance.json"), "status": "prepared"}
            fixture.write_json(evidence / "refresh.json", report)
            next_names = set(following["sourceFiles"])
            for name in sorted(set(before) - next_names):
                (source / name).unlink()
            for name in sorted(next_names | {"FIXTURE-PROVENANCE.json", "NEVER-DEPLOY.txt", "fixture-policy.scale"}):
                output = source / name
                output.parent.mkdir(parents=True, exist_ok=True)
                # Preserve unchanged mtimes so a new policy does not invalidate
                # every warm dependency. The full before/after fingerprints
                # still authenticate every file, including the unchanged ones.
                if not output.exists() or output.read_bytes() != (fresh / name).read_bytes():
                    shutil.copy2(fresh / name, output)
            after = build.fingerprint(source, following)
            build.require(set(after) == next_names, "Refreshed fixture source inventory mismatch")
            report["status"] = "complete"
            report["files"] = len(after)
            fixture.write_json(evidence / "refresh.json", report)
            return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--policy", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    result = refresh(args.source, args.policy, args.evidence.resolve())
    print(json.dumps({"status": result["status"], "source": result["source"], "files": result["files"], "fixtureOnly": True, "productionReady": False}))


if __name__ == "__main__":
    main()
