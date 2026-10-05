#!/usr/bin/env python3
"""Offline integrity, source, evidence, and unsigned-call checks for SORA 4.8.12."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED = {"SHA256SUMS", "validation/package-check.json", "validation/.public-read-cache.json.gz"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load(name):
    return json.loads((ROOT / name).read_text())


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def safe_relative(name):
    relative = Path(name)
    require(not relative.is_absolute() and ".." not in relative.parts, "Unsafe package path: " + name)
    return relative


def compact(value):
    if value < 1 << 6:
        return bytes([value << 2])
    if value < 1 << 14:
        return ((value << 2) | 1).to_bytes(2, "little")
    if value < 1 << 30:
        return ((value << 2) | 2).to_bytes(4, "little")
    size = (value.bit_length() + 7) // 8
    return bytes([((size - 4) << 2) | 3]) + value.to_bytes(size, "little")


def call_index(metadata, pallet_name, call_name):
    pallet = next(p for p in metadata["pallets"] if p["name"] == pallet_name)
    types = {entry["id"]: entry["type"] for entry in metadata["types"]["types"]}
    variants = types[pallet["calls"]["ty"]]["def"]["variant"]["variants"]
    call = next(v for v in variants if v["name"] == call_name)
    return bytes([pallet["index"], call["index"]])


def evidence(name, provenance):
    report = load("validation/" + name + ".json")
    require(report["status"] == "passed", name + " did not pass")
    require(report["sourceTreeAfterPatch"] == provenance["sourceTreeAfterPatch"],
            name + " used another source overlay")
    log = safe_relative(report["log"])
    require(sha256((ROOT / "validation" / log).read_bytes()) == report["logSha256"],
            name + " log changed")
    return report


def verify():
    names = set()
    for line in (ROOT / "SHA256SUMS").read_text().splitlines():
        expected, name = line.split("  ", 1)
        relative = safe_relative(name)
        require(name not in names, "Duplicate manifest path: " + name)
        names.add(name)
        require(sha256((ROOT / relative).read_bytes()) == expected, "Hash mismatch: " + name)
    actual = {str(p.relative_to(ROOT)) for p in ROOT.rglob("*") if p.is_file()
              and not any(part in {"node_modules", "__pycache__"} for part in p.relative_to(ROOT).parts)}
    actual -= EXCLUDED
    require(names == actual, "Unlisted or missing package files: " + str(names ^ actual))

    info = load("runtime-upgrade-4.8.12-info.json")
    candidate = info["candidate"]
    wasm = (ROOT / safe_relative(candidate["file"])).read_bytes()
    digest = sha256(wasm)
    require(candidate["sha256"] == digest and candidate["bytes"] == len(wasm), "Candidate mismatch")
    require(candidate["specVersion"] == 134 and candidate["transactionVersion"] == 131, "Unexpected version")
    require(info["submittedTransactions"] == 0 and info["privateKeysRead"] == 0, "Unexpected live action")

    snapshot = load("validation/snapshot.json")
    require(snapshot["status"] == "passed", "Pinned public snapshot did not pass")
    require(snapshot["inputs"]["scriptSha256"] == sha256((ROOT / "validation/capture-public-state.cjs").read_bytes()), "Snapshot capture script changed")
    require(snapshot["inputs"]["packageLockSha256"] == sha256((ROOT / "validation/package-lock.json").read_bytes()), "Snapshot tooling lock changed")
    require(sha256((ROOT / "validation" / safe_relative(snapshot["code"]["file"])).read_bytes()) == snapshot["code"]["sha256"], "Captured deployed Wasm changed")
    require(sha256((ROOT / "validation" / safe_relative(snapshot["metadata"]["file"])).read_bytes()) == snapshot["metadata"]["sha256"], "Captured deployed metadata changed")
    rehearsal = load("validation/equivocation-fee-wasm-rehearsal.json")
    compatibility = load("validation/wasm-compatibility.json")
    require(rehearsal["status"] == compatibility["status"] == "passed", "Exact-Wasm validation did not pass")
    require(rehearsal["candidate"]["sha256"] == digest, "Rehearsal used another Wasm")
    require(rehearsal["pinnedBlockHash"] == snapshot["blockHash"], "Rehearsal used another state")
    require(all(rehearsal["checks"].values()), "A required exact-Wasm check failed")
    require(len(rehearsal["unsignedChecks"]) == 4 and all(row["passed"] for row in rehearsal["unsignedChecks"]), "Four unsigned report paths were not checked")
    require(len(rehearsal["signedChecks"]) >= 9 and all(row["passed"] and int(row["xorCharged"]) > 0 for row in rehearsal["signedChecks"]), "Signed report fee checks incomplete")
    for field, name in [("scriptSha256", "rehearse-equivocation-fees.cjs"), ("snapshotSha256", "snapshot.json"), ("packageLockSha256", "package-lock.json")]:
        require(rehearsal["inputs"][field] == sha256((ROOT / "validation" / name).read_bytes()), "Rehearsal input changed: " + name)
    require(compatibility["inputs"]["candidate"]["sha256"] == digest, "Compatibility used another Wasm")
    require(compatibility["scriptSha256"] == sha256((ROOT / "validation/verify-wasm-compatibility.cjs").read_bytes()), "Compatibility script changed")
    predecessor_abi = compatibility["predecessorMetadataComparison"]
    require(predecessor_abi["previousCandidateSha256"] == "98f152040b1f084b53f7c2e024c0357c03ca62a89c4b2fd26546a0e446da55e4" and
            predecessor_abi["previousMetadataSha256"] == "b4c21e38685bfff6614e8ca94b2102b002c0eed859573169c3a0a68fb29902b0",
            "Retirement ABI comparison must use the sealed paid-migration candidate")
    require(predecessor_abi["existingEncodingsPreserved"] and predecessor_abi["onlyDeclaredRetirementAdditions"] and predecessor_abi["allOtherAbiAndConstantsPreserved"] and
            predecessor_abi["capacityAbiRetained"], "Retirement changed the predecessor ABI or constants")
    comparator_spec = importlib.util.spec_from_file_location("package_metadata_comparison", ROOT / "validation/compare-metadata.py")
    comparator = importlib.util.module_from_spec(comparator_spec)
    comparator_spec.loader.exec_module(comparator)
    expected_additions = comparator.RETIREMENT_ADDITIONS
    require(len(expected_additions) == 5 and predecessor_abi["allowedRetirementAdditions"] == expected_additions,
            "Retirement additions differ from the exact typed allowlist")
    exact_retirement = comparator.retirement_comparison(
        load("validation/previous-candidate-metadata.json")["V14"],
        load("validation/candidate-metadata.json")["V14"],
    )
    require(exact_retirement["compatibleExistingScaleEncoding"] and exact_retirement["onlyDeclaredRetirementAdditions"] and
            exact_retirement["allOtherAbiAndConstantsPreserved"] and not exact_retirement["constantValueChanges"] and
            not exact_retirement["unexpectedRemainingAdditions"], "Fresh offline retirement ABI/type comparison failed")
    predecessor_report = load("validation/predecessor-metadata-compatibility.json")
    require(predecessor_report["comparisonMode"] == "exact-lending-retirement-additions" and
            predecessor_report["compatibleExistingScaleEncoding"] and
            predecessor_report["allowedRetirementAdditions"] == expected_additions and
            len(predecessor_report["selfChecks"]) >= 26, "Retirement metadata negative checks incomplete")
    require(predecessor_abi["previousMetadataSha256"] == sha256((ROOT / "validation/previous-candidate-metadata.json").read_bytes()),
            "Predecessor metadata changed")
    require(predecessor_abi["candidateMetadataSha256"] == sha256((ROOT / "validation/candidate-metadata.json").read_bytes()),
            "Predecessor comparison used another candidate metadata")
    require(predecessor_abi["reportSha256"] == sha256((ROOT / "validation/predecessor-metadata-compatibility.json").read_bytes()),
            "Predecessor ABI comparison report changed")
    predecessor = load("validation/predecessor-context.json")
    require(predecessor["status"] == "retained-historical-evidence" and not predecessor["freshFollowupExecutionClaimed"] and
            predecessor["candidate"]["sha256"] == predecessor_abi["previousCandidateSha256"] and
            predecessor["metadataSha256"] == predecessor_abi["previousMetadataSha256"], "Predecessor context mismatch")
    require(predecessor["historicalCapacityComparisonSha256"] ==
            sha256((ROOT / "validation/historical-capacity-metadata-compatibility.json").read_bytes()),
            "Historical capacity comparison changed")
    require(compatibility["snapshotSha256"] == sha256((ROOT / "validation/snapshot.json").read_bytes()), "Compatibility snapshot changed")
    require(compatibility["tools"]["comparatorSha256"] == sha256((ROOT / "validation/compare-metadata.py").read_bytes()), "Metadata comparator changed")
    require(compatibility["inputs"]["baseline"]["sha256"] == snapshot["code"]["sha256"] == info["preflight"]["deployedSha256"], "Baseline mismatch")
    require(all(compatibility["checks"].values()), "Compatibility check failed")
    fee_events = load("validation/fee-event-check.json")
    require(fee_events["status"] == "passed" and fee_events["candidateSha256"] == digest and
            fee_events["checkedSignedReports"] == 9 and fee_events["allFeeEventsMatchPayerBalanceDecrease"] and
            fee_events["allFailedReportsAreDuplicateOffenceReport"] and fee_events["networkRequests"] == 0,
            "Independent retained-fee event checks incomplete")
    require(fee_events["rehearsalSha256"] == sha256((ROOT / "validation/equivocation-fee-wasm-rehearsal.json").read_bytes()), "Fee event check used another rehearsal")
    require(fee_events["scriptSha256"] == sha256((ROOT / "validation/verify-fee-events.py").read_bytes()), "Fee event check script changed")

    policy = load("validation/fee-policy-wasm-rehearsal.json")
    require(policy["status"] == "passed" and policy["candidate"]["sha256"] == digest,
            "Fee policy rehearsal did not pass on the exact candidate")
    require(policy["pinnedBlockHash"] == snapshot["blockHash"] and all(policy["checks"].values()),
            "Fee policy state or required checks differ")
    required_policy_checks = {
        "exactCandidateWasmUpgradeExecuted", "allEmptyCancellationShapesRetainFees",
        "unauthorizedRewardsRetainsFees", "signedInvalidKensetsuAndApolloRetainFees",
        "bareKensetsuAndApolloReject", "invalidInboundProofsReject",
        "allCheckedFeeEventsMatchBalances", "requestedPreimageFirstAndReplay",
        "successfulCancellationAndPaidReplay", "zeroXorLegacyPeerAndReplayRejection",
        "bridgeCapacityFairQuotaProtectsHonestPeers", "bridgeCapacityFullQueueQuorumCleanup",
        "bridgeCapacityCleanupPreservesProtocolAndRejectsReplay",
        "paidMigrationSuccessAndFailure", "zeroXorMigrationRejectsBeforeExecution",
        "retiredLendingExitsAndGuards",
        "protectedStakingStateAndXorIssuancePreservedByUpgrade", "noPublicTransactionSubmission",
    }
    require(required_policy_checks <= set(policy["checks"]), "Missing required fee policy coverage")
    require(policy["submittedTransactions"] == 0 and policy["privateKeysRead"] == 0,
            "Unexpected action in fee policy rehearsal")
    for field, name in [("scriptSha256", "rehearse-fee-policy.cjs"), ("snapshotSha256", "snapshot.json"), ("packageLockSha256", "package-lock.json")]:
        require(policy["inputs"][field] == sha256((ROOT / "validation" / name).read_bytes()),
                "Fee policy rehearsal input changed: " + name)

    require(policy["inputs"]["retirementHelperSha256"] ==
            sha256((ROOT / "validation/retired-lending-fixture.cjs").read_bytes()),
            "Retired lending Wasm fixture changed")
    retirement = policy["retiredLending"]
    require(retirement["passed"] and retirement["synthetic"] and
            retirement["constantChecks"] == {"kensetsu": True, "apolloPlatform": True},
            "Exact-Wasm lending retirement mode checks incomplete")
    kensetsu = retirement.get("kensetsu", {})
    require(kensetsu.get("debtPolicy") == "existing-interest-unchanged" and
            kensetsu.get("nonzeroRateInterestAccrues") is True and
            kensetsu.get("existingTreasuryAccountingPreserved") is True,
            "Exact-Wasm proof of unchanged Kensetsu interest and treasury accounting is missing")
    blocked_retirement_calls = {
        "kensetsu.createCdp", "kensetsu.depositCollateral", "kensetsu.borrow",
        "kensetsu.accrue", "kensetsu.liquidate", "kensetsu.donate",
        "apolloPlatform.lend", "apolloPlatform.borrow", "apolloPlatform.addCollateral", "apolloPlatform.liquidate",
    }
    expected_blocked = {name + ":" + route for name in blocked_retirement_calls for route in ["direct", "batchAll"]}
    require(len(retirement["blockedCalls"]) == 20 and set(retirement["blockedCalls"]) == expected_blocked,
            "Exact-Wasm retirement must check every blocked direct and batched call")
    for name in blocked_retirement_calls:
        for route in ["direct", "batchAll"]:
            rows = [row for row in policy["signedChecks"] if row["method"] == name + " retirement " + route]
            require(len(rows) == 1 and rows[0]["passed"] and rows[0]["decodedError"] == "RepaymentOnly" and
                    not rows[0]["expectSuccess"] and not rows[0]["expectFree"] and
                    int(rows[0]["xorCharged"]) > 0 and rows[0]["actualFeeEventVerified"],
                    "Blocked retirement call did not retain its fee: " + name + " " + route)
    for name in ["kensetsu.accrue", "kensetsu.liquidate", "apolloPlatform.liquidate"]:
        rows = [row for row in policy["unsignedChecks"] if row["method"] == name + " retirement bare maintenance"]
        require(len(rows) == 1 and rows[0]["passed"] and
                set(rows[0]["sources"]) == {"External", "Local", "InBlock"} and
                all("err" in value for value in rows[0]["sources"].values()) and "err" in rows[0]["apply"],
                "Retired unsigned maintenance admission was not rejected: " + name)
    require(all(retirement["kensetsu"][key] is True for key in [
        "paidPartialRepayment", "paidClosure", "collateralReturned", "debtAndOwnerIndexRemoved", "stablecoinBurnMatchesDebt"]),
        "Exact-Wasm Kensetsu retirement exit incomplete")
    require(retirement["kensetsu"]["principal"] == str(100 * 10 ** 18) and
            retirement["kensetsu"]["collateral"] == str(200 * 10 ** 18), "Kensetsu exit fixture amounts differ")
    require(all(retirement["apollo"][key] is True for key in [
        "paidPartialAndFullRepayment", "collateralReturned", "recordedInterestReserved", "reserveExcludedFromLenderWithdrawal",
        "exactLastLenderWithdrawal", "principalExitWithoutRewardFunding", "earnedClaimsPreserved", "failedClaimRollsBack",
        "retainedClaimsRedeemAfterFunding"]), "Exact-Wasm Apollo retirement accounting/exit incomplete")
    paid_migration = policy["paidMigration"]
    require(paid_migration["passed"] and paid_migration["ownershipProofVerifiedByWasm"] and
            paid_migration["successRetainsXorFee"] and paid_migration["failureRetainsXorFee"] and
            paid_migration["failureRollsBackValAndClaim"] and paid_migration["replayPaidWithoutDuplicateVal"] and
            paid_migration["sponsorshipAbiAbsent"], "Exact Wasm paid migration incomplete")
    require(paid_migration["syntheticProofSha256"] == sha256((ROOT / "validation/migration-proof-fixture.json").read_bytes()) and
            paid_migration["proofGeneratorSha256"] == sha256((ROOT / "validation/migration-proof-fixture.rs").read_bytes()),
            "Paid migration ownership-proof inputs changed")
    zero_migration = policy["zeroXorMigration"]
    require(zero_migration["passed"] and zero_migration["claimBalancesAndNoncePreserved"] and
            set(zero_migration["sources"]) == {"External", "Local", "InBlock"} and
            all(row == {"err": {"invalid": {"payment": None}}} for row in zero_migration["sources"].values()) and
            zero_migration["apply"] == {"err": {"invalid": {"payment": None}}}, "Zero-XOR migration admission was not rejected before dispatch")
    bridge = load("validation/bridge-readiness.json")
    require(bridge["status"] == "passed" and bridge["submittedTransactions"] == 0
            and bridge["legacyBacklogGrandfathered"] and bridge["newOperationLimitUsesAdditiveTracking"]
            and bridge["fairPerProposerQuotaConfigured"] and bridge["currentPeerQuorumCancellationConfigured"],
            "Bridge readiness inspection incomplete")
    require(bridge["scriptSha256"] == sha256((ROOT / "validation/check-bridge-readiness.cjs").read_bytes()),
            "Bridge readiness script changed")
    require(bridge["snapshotSha256"] == sha256((ROOT / "validation/snapshot.json").read_bytes()),
            "Bridge readiness uses another baseline snapshot")

    governance = load("validation/governance-call-check.json")
    require(governance["status"] == "passed", "Unsigned governance decoding did not pass")
    require(governance["candidateSha256"] == digest and governance["setCodeProposalHash"] == info["proposal"]["hash"], "Governance refers to another candidate")
    require(governance["verifiedCalls"] == 6 and governance["networkRequests"] == 0 and
            governance["guardedCouncilExactTwoCallOrderThresholdLengthAndHashChecked"] and
            governance["councilSettingsMatchDecodedCallsAndFinalizedPreflight"], "Guarded unsigned governance checks incomplete")
    require(governance["existingExternalQueuePreserved"] and governance["noDirectSupersessionArtifacts"], "Unsafe governance supersession route")
    for name, expected in governance["inputSha256"].items():
        require(sha256((ROOT / safe_relative(name)).read_bytes()) == expected, "Governance input changed: " + name)

    provenance = load("validation/source-provenance.json")
    require(provenance["status"] == "passed" and provenance["sourceIsUncommittedOverlay"] is True and
            provenance["patchAppliesToCleanBase"] and provenance["replayedTreeMatches"] and
            provenance["userIndexModified"] is False, "Exact source overlay was not verified")
    require(sha256((ROOT / safe_relative(provenance["patch"])).read_bytes()) == provenance["patchSha256"], "Source patch changed")
    native = evidence("native-tests", provenance)
    build = evidence("wasm-build", provenance)
    lint = evidence("clippy", provenance)
    require(lint["exitCode"] == 0 and lint["profiles"] == ["mainnet", "try-runtime", "extended"],
            "Fresh three-profile Clippy evidence incomplete")
    require(native["totalFailed"] == 0 and native["newRuntimeRegressionsPassed"] >= 39, "Native fee policy regressions incomplete")
    require(native["capacityExecutiveRegressionsPassed"] == 2, "Capacity/quorum Executive regressions incomplete")
    required_followup_tests = {
        "tests::liveness::bridge_fees::outgoing_approval_retains_validation_weight_before_and_at_quorum",
        "tests::liveness::migration_fees::migration_success_delivers_val_and_keeps_xor_fee",
        "tests::liveness::migration_fees::migration_settlement_failure_keeps_fee_and_rolls_back_claim",
        "tests::liveness::migration_fees::migration_requires_xor_before_valid_claim_can_execute",
        "tests::liveness::migration_fees::migration_invalid_proof_and_replay_pay_without_duplicate_val",
        "tests::liveness::migration_fees::wrapped_migration_requires_xor_and_keeps_success_fee",
    }
    require(native["followupRegressionsPassed"] == 6 and
            set(native["requiredFollowupRegressions"]) == required_followup_tests,
            "Bridge weight and paid migration regressions incomplete")
    required_retirement_tests = {
        "tests::liveness::retired_lending::retired_kensetsu_operations_are_paid_failures_even_in_batches",
        "tests::liveness::retired_lending::retired_apollo_operations_are_paid_failures_even_in_batches",
        "tests::liveness::retired_lending::retired_kensetsu_partial_repayment_and_close_unlock_existing_collateral",
        "tests::liveness::retired_lending::retired_apollo_repayment_unlocks_existing_collateral",
        "tests::liveness::retired_lending::retired_apollo_last_lender_withdraws_exact_remaining_liquidity",
        "tests::liveness::retired_lending::retired_apollo_exit_preserves_unfunded_reward_claims",
        "tests::liveness::retired_lending::retired_lending_workers_submit_nothing_with_funded_keeper_keys",
    }
    require(native["retirementRegressionsPassed"] == 7 and
            set(native["requiredRetirementRegressions"]) == required_retirement_tests,
            "Native lending retirement Executive regressions incomplete")
    require(native["requiredKensetsuInterestRegression"] == "tests::repayment_only_preserves_interest_and_treasury_accounting_during_owner_exit",
            "Native continuing-interest regression is missing")
    require(len(native["suites"]) == 13 and {"kensetsu", "apollo_platform"} <= {row["crate"] for row in native["suites"]},
            "Fresh native evidence must include both retired lending pallet suites")
    require(provenance["baseCommit"] == "bb38216396835fae45de9c38104e4927a96eb36c",
            "Follow-up source does not use the reviewed bb382163 base")
    require(set(provenance["requiredFollowupInputs"]) <= set(provenance["files"]),
            "Follow-up production and regression sources are not bound")
    required_deletions = {"runtime/src/migration_fees.rs", "runtime/src/tests/liveness/migration_sponsorship.rs"}
    require(set(provenance["requiredDeletions"]) == required_deletions and
            required_deletions <= set(provenance["deletedFiles"]), "Sponsorship source deletions not captured")
    if "--check-workspace-source" in sys.argv[1:]:
        for name, before_sha in provenance["deletedFiles"].items():
            require(not (ROOT.parent / safe_relative(name)).exists(), "Deleted source restored: " + name)
            before = subprocess.check_output(["git", "show", provenance["baseCommit"] + ":" + name], cwd=ROOT.parent)
            require(sha256(before) == before_sha, "Deleted-source baseline hash differs: " + name)
    chain_specs = load("validation/chain-spec-runtimes.json")
    require(chain_specs["schemaVersion"] == 1 and chain_specs["status"] == "passed" and
            chain_specs["sourceTree"] == provenance["sourceTreeAfterPatch"] and
            chain_specs["sourceBaseCommit"] == provenance["baseCommit"] and
            chain_specs["chainSpecBaseCommit"] == "3b95216336fb4baf419ec4271f97c09e66f3579f" and not chain_specs["networkUsed"],
            "Regenerated chain specs used another source or a network service")
    require(chain_specs["provenanceSha256"] == sha256((ROOT / "validation/source-provenance.json").read_bytes()) and
            chain_specs["scriptSha256"] == sha256((ROOT / "validation/refresh-chain-specs.py").read_bytes()) and
            chain_specs["comparatorSha256"] == sha256((ROOT / "validation/compare-metadata.py").read_bytes()),
            "Chain-spec regeneration inputs changed")
    expected_profiles = {
        "stage": {"build-wasm-binary", "private-net", "stage"},
        "test": {"build-wasm-binary", "private-net", "stage", "wip", "reduced-pswap-reward-periods"},
    }
    expected_specs = {
        "stage": {"node/chain_spec/src/bytes/chain_spec_staging.json", "node/chain_spec/src/bytes/chain_spec_bridge_staging.json"},
        "test": {"node/chain_spec/src/bytes/chain_spec_test.json"},
    }
    require({row["profile"] for row in chain_specs["runtimes"]} == set(expected_profiles) and
            len(chain_specs["runtimes"]) == 2, "Chain-spec runtime profiles differ")
    for runtime in chain_specs["runtimes"]:
        profile = runtime["profile"]
        require(set(runtime["features"]) == expected_profiles[profile] and
                runtime["sourceTree"] == provenance["sourceTreeAfterPatch"], "Chain-spec feature/source mismatch")
        wasm_info = runtime["wasm"]
        require(wasm_info["specVersion"] == 134 and wasm_info["transactionVersion"] == 131 and
                len(wasm_info["sha256"]) == 64 and wasm_info["bytes"] > 0,
                "Chain-spec runtime version or fingerprint differs")
        require(all(runtime["checks"].values()) and runtime["checks"]["cancelPendingMultisigCall18"] and
                runtime["checks"]["capacityStoragePresent"] and runtime["checks"]["metadataSelfChecksPassed"] and
                len(runtime["metadataSelfChecks"]) >= 9, "Chain-spec runtime lacks current capacity ABI or strict comparator checks")
        require({row["path"] for row in runtime["specs"]} == expected_specs[profile], "Chain-spec set differs")
        for spec in runtime["specs"]:
            require(spec["sourceBaseCommit"] == provenance["baseCommit"] and
                    spec["chainSpecBaseCommit"] == chain_specs["chainSpecBaseCommit"] and
                    spec["codeSha256"] == wasm_info["sha256"] and spec["codeBytes"] == wasm_info["bytes"] and
                    spec["nonCodeValuesUnchanged"] and spec["nonCodeBytesUnchanged"] and
                    spec["lastRuntimeUpgradeUnchanged"] and spec["palletIndicesPreserved"] and
                    spec["compatibleExistingScaleEncoding"] and spec["constantValuesPreserved"],
                    "Chain-spec content/ABI preservation failed")
            require(spec["onlyDeclaredRetirementAdditions"] and
                    spec["metadataComparison"]["allowedRetirementAdditions"] == expected_additions and
                    spec["metadataComparison"]["allOtherAbiAndConstantsPreserved"] and
                    not spec["metadataComparison"]["unexpectedRemainingAdditions"] and
                    not spec["metadataComparison"]["constantValueChanges"] and
                    len(spec["retirementSelfChecks"]) >= 17,
                    "Feature ABI changed beyond the exact typed retirement additions")
            require(spec["currentMetadataSha256"] == wasm_info["metadataSha256"], "Chain-spec metadata fingerprint differs")
            if "--check-workspace-source" in sys.argv[1:]:
                path = ROOT.parent / safe_relative(spec["path"])
                raw = path.read_bytes()
                require(sha256(raw) == spec["sha256"] and len(raw) == spec["bytes"], "Workspace chain spec changed: " + spec["path"])
                code = bytes.fromhex(json.loads(raw)["genesis"]["raw"]["top"]["0x3a636f6465"][2:])
                require(sha256(code) == spec["codeSha256"] and len(code) == spec["codeBytes"], "Workspace embedded runtime changed: " + spec["path"])
    owned = load("validation/owned-tests.json")
    historical_owned = load("validation/historical-owned-tests-context.json")
    require(historical_owned["status"] == "retained-historical-evidence" and
            historical_owned["freshCapacityFixExecutionClaimed"] is False and
            historical_owned["originalSourceTreeAfterPatch"] == "a16e9e1ac195575c6008c69434999b2587b81d01",
            "Historical owned-test evidence was relabeled as fresh capacity execution")
    require(owned["status"] == "passed" and owned["tests_failed"] == 0 and owned["tests_passed"] >= 430,
            "Maintenance and inbound bridge unit evidence incomplete")
    for name, expected in owned["validation_files"].items():
        require(sha256((ROOT / "validation" / safe_relative(name)).read_bytes()) == expected,
                "Owned unit-test evidence changed: " + name)
    require(owned["production_sdk_and_lock_provenance"]["root_cargo_lock_sha256"] == provenance["files"]["Cargo.lock"]
            and owned["production_sdk_and_lock_provenance"]["nested_cargo_lock_sha256"] == provenance["files"]["vendor/sora2-common/Cargo.lock"],
            "Owned unit tests used different locked dependencies")
    require(build["candidateSha256"] == digest and build["candidateBytes"] == len(wasm), "Build evidence used another Wasm")
    require(build["wasmPackagesMatchSourceLock"] is True and
            build["cargoLockSha256"] == provenance["files"]["Cargo.lock"], "Wasm dependencies do not match exact source lock")
    if "--check-workspace-source" in sys.argv[1:]:
        for name, expected in provenance["files"].items():
            require(sha256((ROOT.parent / safe_relative(name)).read_bytes()) == expected, "Workspace source changed: " + name)
        with tempfile.TemporaryDirectory(prefix="sora-4812-verify-") as temporary:
            env = {**os.environ, "GIT_INDEX_FILE": str(Path(temporary) / "index")}
            def git(*args):
                return subprocess.check_output(["git", *args], cwd=ROOT.parent, env=env).decode().strip()
            git("read-tree", provenance["baseCommit"])
            git("apply", "--cached", "--whitespace=nowarn", str(ROOT / provenance["patch"]))
            require(git("write-tree") == provenance["sourceTreeAfterPatch"], "Source replay produced another tree")

    docs = load("validation/docs-evidence.json")
    require(docs["status"] == "passed" and docs["candidateSha256"] == digest and
            docs["sourceTreeAfterPatch"] == provenance["sourceTreeAfterPatch"], "Final documentation used stale evidence")
    require(docs["scriptSha256"] == sha256((ROOT / "validation/finalize-docs.py").read_bytes()), "Documentation generator changed")
    require(set(docs["documents"]) == {"README.md", "VALIDATION.md", "GOVERNANCE-RUNBOOK.md", "COUNCIL_MESSAGE.md"}, "Required documentation missing")
    for name, expected in docs["documents"].items():
        data = (ROOT / safe_relative(name)).read_bytes()
        require(sha256(data) == expected and b"{{" not in data, "Stale or unrendered documentation: " + name)
    for name, expected in docs["inputSha256"].items():
        require(sha256((ROOT / safe_relative(name)).read_bytes()) == expected, "Documentation input changed: " + name)

    metadata = load("validation/baseline-metadata.json")["V14"]
    encoded = call_index(metadata, "System", "set_code") + compact(len(wasm)) + wasm
    require((ROOT / "set-code-call.scale").read_bytes() == encoded, "setCode SCALE mismatch")
    require(bytes.fromhex((ROOT / "set-code-call.hex").read_text().strip()[2:]) == encoded, "setCode hex mismatch")
    require(info["proposal"]["bytes"] == len(encoded), "Proposal length mismatch")
    proposal_hash = "0x" + hashlib.blake2b(encoded, digest_size=32).hexdigest()
    require(info["proposal"]["hash"] == proposal_hash, "Proposal hash mismatch")
    preimage = call_index(metadata, "Preimage", "note_preimage") + compact(len(encoded)) + encoded
    require(bytes.fromhex((ROOT / "preimage-note-call.hex").read_text().strip()[2:]) == preimage, "Preimage mismatch")
    report = {"status": "passed", "verifiedFiles": len(names), "candidateSha256": digest,
              "proposalHash": proposal_hash, "sourceTreeAfterPatch": provenance["sourceTreeAfterPatch"],
              "sourceIsUncommittedOverlay": True, "unsignedCallsMatchCandidate": True,
              "activationReady": governance["activationReady"], "existingExternalQueuePreserved": True,
              "exactWasmReportFeesAndCompatibilityPassed": True, "nativeReportFeeTestsPassed": True,
              "exactWasmFeePolicyAndZeroXorBridgePassed": True,
              "nativeAndExactWasmLendingRetirementPassed": True,
              "workspaceSourceChecked": "--check-workspace-source" in sys.argv[1:], "networkRequests": 0}
    (ROOT / "validation/package-check.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    verify()
