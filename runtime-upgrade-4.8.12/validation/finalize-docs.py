#!/usr/bin/env python3
"""Render release documentation from exact recorded evidence, without changing logs."""
import argparse
import hashlib
import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--require-final-gates", action="store_true")
args = parser.parse_args()
read = lambda name: json.loads((ROOT / name).read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
info = read("runtime-upgrade-4.8.12-info.json")
provenance = read("validation/source-provenance.json")
native = read("validation/native-tests.json")
build = read("validation/wasm-build.json")
policy = read("validation/fee-policy-wasm-rehearsal.json")
compatibility = read("validation/wasm-compatibility.json")
governance = read("validation/governance-call-check.json")
settings = read("council-settings.json")
candidate = info["candidate"]
assert candidate["sha256"] == sha((ROOT / candidate["file"]).read_bytes())
for report in [native, build, policy, compatibility, governance]:
    assert report["status"] == "passed"
assert native["sourceTreeAfterPatch"] == build["sourceTreeAfterPatch"] == provenance["sourceTreeAfterPatch"]
assert build["candidateSha256"] == policy["candidate"]["sha256"] == compatibility["inputs"]["candidate"]["sha256"] == governance["candidateSha256"] == candidate["sha256"]
assert native["totalFailed"] == 0 and native["followupRegressionsPassed"] == 3
chain_path = HERE / "chain-spec-runtimes.json"
lint_path = HERE / "clippy.json"
chain = json.loads(chain_path.read_text()) if chain_path.exists() else {}
lint = json.loads(lint_path.read_text()) if lint_path.exists() else {}
chain_passed = chain.get("status") == "passed" and chain.get("sourceTree") == provenance["sourceTreeAfterPatch"]
lint_passed = lint.get("status") == "passed" and lint.get("sourceTreeAfterPatch") == provenance["sourceTreeAfterPatch"]
if args.require_final_gates:
    assert chain_passed and lint_passed, "Fresh chain-spec and Clippy gates must pass"
values = {
    "CANDIDATE_SHA256": candidate["sha256"], "CANDIDATE_BYTES": f'{candidate["bytes"]:,}',
    "SOURCE_TREE": provenance["sourceTreeAfterPatch"], "SOURCE_BASE": provenance["baseCommit"],
    "NATIVE_PASSED": f'{native["totalPassed"]:,}', "NATIVE_IGNORED": str(native["totalIgnored"]),
    "RUNTIME_REGRESSIONS": str(native["newRuntimeRegressionsPassed"]),
    "POLICY_SIGNED": str(len(policy["signedChecks"])),
    "POLICY_PAID": str(sum(int(row["xorCharged"]) > 0 for row in policy["signedChecks"])),
    "POLICY_FREE": str(sum(int(row["xorCharged"]) == 0 for row in policy["signedChecks"])),
    "PREFLIGHT_BLOCK": f'{settings["network"]["finalizedBlock"]:,}',
    "PROPOSAL_HASH": info["proposal"]["hash"], "PROPOSAL_BYTES": f'{info["proposal"]["bytes"]:,}',
    "COUNCIL_THRESHOLD": str(settings["council"]["threshold"]),
    "TECHNICAL_THRESHOLD": str(settings["technicalCommittee"]["threshold"]),
    "CHAIN_SPEC_STATUS": "passed" if chain_passed else "pending",
    "CLIPPY_STATUS": "passed" if lint_passed else "pending",
}
documents = {}
for name in ["README.md", "VALIDATION.md", "GOVERNANCE-RUNBOOK.md", "COUNCIL_MESSAGE.md"]:
    text = (HERE / "docs" / (name + ".in")).read_text()
    text = re.sub(r"\{\{([A-Z0-9_]+)\}\}", lambda match: values[match[1]], text)
    assert "{{" not in text, "Unknown documentation placeholder"
    (ROOT / name).write_text(text)
    documents[name] = sha((ROOT / name).read_bytes())
inputs = ["runtime-upgrade-4.8.12-info.json", "council-settings.json",
          "validation/source-provenance.json", "validation/native-tests.json", "validation/wasm-build.json",
          "validation/fee-policy-wasm-rehearsal.json", "validation/wasm-compatibility.json",
          "validation/governance-call-check.json", "validation/clippy.json"]
if chain_path.exists():
    inputs.append("validation/chain-spec-runtimes.json")
inputs += ["validation/docs/" + name + ".in" for name in documents]
report = {"status": "passed" if chain_passed and lint_passed else "pending-final-gates",
          "sourceTreeAfterPatch": provenance["sourceTreeAfterPatch"], "candidateSha256": candidate["sha256"],
          "scriptSha256": sha(Path(__file__).read_bytes()), "documents": documents,
          "inputSha256": {name: sha((ROOT / name).read_bytes()) for name in inputs}}
(HERE / "docs-evidence.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps({"status": report["status"], "candidateSha256": candidate["sha256"],
                  "nativePassed": native["totalPassed"], "runtimeRegressions": native["newRuntimeRegressionsPassed"]}))
