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
assert native["totalFailed"] == 0 and native["followupRegressionsPassed"] == 6
required_retirement_tests = {
    "tests::liveness::retired_lending::retired_kensetsu_operations_are_paid_failures_even_in_batches",
    "tests::liveness::retired_lending::retired_apollo_operations_are_paid_failures_even_in_batches",
    "tests::liveness::retired_lending::retired_kensetsu_partial_repayment_and_close_unlock_existing_collateral",
    "tests::liveness::retired_lending::retired_apollo_repayment_unlocks_existing_collateral",
    "tests::liveness::retired_lending::retired_apollo_last_lender_withdraws_exact_remaining_liquidity",
    "tests::liveness::retired_lending::retired_apollo_exit_preserves_unfunded_reward_claims",
    "tests::liveness::retired_lending::retired_lending_workers_submit_nothing_with_funded_keeper_keys",
}
assert native.get("retirementRegressionsPassed") == 7 and set(native.get("requiredRetirementRegressions", [])) == required_retirement_tests, "Fresh lending retirement regressions must pass before documentation is rendered"
assert native["newRuntimeRegressionsPassed"] >= 39 and len(native["suites"]) == 13
assert {"kensetsu", "apollo_platform"} <= {row["crate"] for row in native["suites"]}
assert all(row["failed"] == 0 for row in native["suites"])
assert policy.get("checks", {}).get("retiredLendingExitsAndGuards") is True and policy.get("retiredLending", {}).get("passed") is True, "Exact-Wasm lending retirement evidence must pass before documentation is rendered"
assert policy["retiredLending"]["constantChecks"] == {"kensetsu": True, "apolloPlatform": True}
assert policy["inputs"]["retirementHelperSha256"] == sha((HERE / "retired-lending-fixture.cjs").read_bytes()), "Exact-Wasm retirement helper changed"
kensetsu = policy["retiredLending"].get("kensetsu", {})
assert kensetsu.get("debtPolicy") == "existing-interest-unchanged" and kensetsu.get("nonzeroRateInterestAccrues") is True and kensetsu.get("existingTreasuryAccountingPreserved") is True, "Existing Kensetsu interest and treasury accounting must be proven by the exact Wasm before release documentation"
kensetsu_policy_descriptions = {
    "existing-interest-unchanged": "Existing Kensetsu debt continues accruing interest under existing terms. Repayment or closure books accrued interest with the existing treasury accounting; no new borrowing is permitted.",
}
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
    "KENSETSU_DEBT_POLICY": kensetsu_policy_descriptions[kensetsu["debtPolicy"]],
}
rendered_documents = {}
for name in ["README.md", "VALIDATION.md", "GOVERNANCE-RUNBOOK.md", "COUNCIL_MESSAGE.md"]:
    text = (HERE / "docs" / (name + ".in")).read_text()
    unresolved = set(re.findall(r"\{\{([A-Z0-9_]+)\}\}", text)) - values.keys()
    assert not unresolved, "Unresolved release policy or documentation value in " + name + ": " + ", ".join(sorted(unresolved))
    text = re.sub(r"\{\{([A-Z0-9_]+)\}\}", lambda match: values[match[1]], text)
    assert "{{" not in text, "Unknown documentation placeholder"
    rendered_documents[name] = text

# No document is overwritten until every template and evidence gate validates.
documents = {}
for name, text in rendered_documents.items():
    (ROOT / name).write_text(text)
    documents[name] = sha((ROOT / name).read_bytes())
inputs = ["runtime-upgrade-4.8.12-info.json", "council-settings.json",
          "validation/source-provenance.json", "validation/native-tests.json", "validation/wasm-build.json",
          "validation/fee-policy-wasm-rehearsal.json", "validation/wasm-compatibility.json",
          "validation/governance-call-check.json", "validation/clippy.json",
          "validation/retired-lending-fixture.cjs"]
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
