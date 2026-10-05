#!/usr/bin/env python3
"""Create and independently verify a review ZIP from an already verified package."""
import hashlib
import json
import argparse
from pathlib import Path
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("output", nargs="?")
parser.add_argument("--check-workspace-source", action="store_true")
args = parser.parse_args()
OUTPUT = Path(args.output).resolve() if args.output else ROOT.parent / "council-runtime-4.8.12.zip"
verification_args = ["--check-workspace-source"] if args.check_workspace_source else []
subprocess.run([sys.executable, str(ROOT / "validation/verify-package.py"), *verification_args], check=True)
manifest = {}
for line in (ROOT / "SHA256SUMS").read_text().splitlines():
    digest, name = line.split("  ", 1)
    manifest[name] = digest
files = sorted([*manifest, "SHA256SUMS", "validation/package-check.json"])
with zipfile.ZipFile(OUTPUT, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    for name in files:
        archive.write(ROOT / name, ROOT.name + "/" + name)
with zipfile.ZipFile(OUTPUT) as archive:
    assert archive.testzip() is None
    assert set(archive.namelist()) == {ROOT.name + "/" + name for name in files}
    for name in files:
        assert archive.read(ROOT.name + "/" + name) == (ROOT / name).read_bytes()
digest = hashlib.sha256(OUTPUT.read_bytes()).hexdigest()
OUTPUT.with_name(OUTPUT.name + ".sha256").write_text(digest + "  " + OUTPUT.name + "\n")
report = {"status": "passed", "release": "4.8.12", "archive": OUTPUT.name,
          "sha256": digest, "bytes": OUTPUT.stat().st_size, "files": len(files),
          "zipCrcAndExactEntriesChecked": True, "allArchivedBytesMatchVerifiedPackage": True,
          "submittedTransactions": 0, "privateKeysRead": 0}
OUTPUT.with_name(OUTPUT.name + ".verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report))
