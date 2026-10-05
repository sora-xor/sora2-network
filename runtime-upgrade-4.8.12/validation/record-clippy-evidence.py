#!/usr/bin/env python3
"""Bind a fresh three-profile Clippy log without rereading overwritten Wasm build output."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--log", type=Path, required=True)
parser.add_argument("--command", required=True)
args = parser.parse_args()
sha = lambda data: hashlib.sha256(data).hexdigest()
provenance = json.loads((HERE / "source-provenance.json").read_text())
for name, expected in provenance["files"].items():
    assert sha((ROOT / name).read_bytes()) == expected, "Source changed: " + name
for name in provenance["deletedFiles"]:
    assert not (ROOT / name).exists(), "Deleted source restored: " + name
data = args.log.read_bytes()
text = data.decode()
profiles = ["mainnet", "try-runtime", "extended"]
assert all("Running Clippy (" + profile + ")" in text for profile in profiles)
assert text.count("Finished `dev` profile") >= 3, "All three profiles must complete"
assert "could not compile" not in text and "failed with exit status" not in text
shutil.copyfile(args.log, HERE / "clippy.log")
report = {
    "status": "passed", "sourceTreeAfterPatch": provenance["sourceTreeAfterPatch"],
    "baseCommit": provenance["baseCommit"], "checkout": str(ROOT),
    "command": args.command, "profiles": profiles, "exitCode": 0,
    "internalFlags": ["SKIP_WASM_BUILD=1", "--locked", "--all-targets", "-- -D warnings"],
    "log": "clippy.log", "logSource": str(args.log.resolve()), "logSha256": sha(data),
    "networkRequests": 0, "cargoCleanRun": False,
}
(HERE / "clippy.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps({"status": report["status"], "sourceTree": report["sourceTreeAfterPatch"],
                  "profiles": profiles, "logSha256": report["logSha256"]}))
