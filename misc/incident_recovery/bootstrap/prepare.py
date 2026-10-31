#!/usr/bin/env python3
"""Export the exact deployed spec-134 source, then apply a reviewed recovery overlay.

No checkout mutation, network access, Cargo resolution or release publication is
performed. An unbound export intentionally cannot execute the first recovery child.
"""
from __future__ import annotations
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile

LEGACY_TREE = "36c466398e3f2760e8ab44fdb89de796f5b0dd99"
ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
SOURCE_PATHS = [".cargo", ".hooks", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "LICENSE",
                "common", "runtime", "pallets", "vendor", "node", "utils", "integration-tests", "misc"]

def digest(data: bytes) -> dict[str, str]:
    return {"sha256": hashlib.sha256(data).hexdigest(), "blake2_256": hashlib.blake2b(data, digest_size=32).hexdigest()}

def replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise ValueError(f"pinned source marker occurs {source.count(old)} times: {old[:90]!r}")
    return source.replace(old, new, 1)

def patch_runtime(source: str, spec: int) -> str:
    if not re.search(r"spec_version:\s*134,", source) or not re.search(r"transaction_version:\s*131,", source):
        raise ValueError("source is not the deployed spec-134/transaction-131 runtime")
    source = replace_once(source, "mod babe_config;", "mod babe_config;\nmod incident_recovery;\nmod incident_recovery_consensus;\nmod incident_recovery_babe_reports;\npub mod incident_recovery_evm_source;\npub mod incident_recovery_incoming;\npub mod incident_recovery_outgoing;")
    for method in ["execute_block(block)", "initialize_block(header)", "apply_extrinsic(extrinsic)", "finalize_block()",
                   "validate_transaction(source, tx, block_hash)", "offchain_worker(header)"]:
        source = replace_once(source, f"Executive::{method}", f"incident_recovery::{method}")
    source = replace_once(source, "            data.create_extrinsics()", "            incident_recovery::inherent_extrinsics(data)")
    source = replace_once(source, "            data.check_extrinsics(&block)", "            incident_recovery::check_inherents(block, data)")
    marker = "        liveness::EquivocationReports,\n        Historical,\n        ReportLongevity,"
    if source.count(marker) != 2:
        raise ValueError("unexpected BABE/GRANDPA report adapter layout")
    source = source.replace(marker, "        liveness::EquivocationReports,\n        incident_recovery_consensus::RecoveryOwnership,\n        ReportLongevity,")
    source = replace_once(source,
        "    type EquivocationReportSystem = pallet_grandpa::EquivocationReportSystem<\n"
        "        Self,\n        liveness::EquivocationReports,\n"
        "        incident_recovery_consensus::RecoveryOwnership,\n        ReportLongevity,\n    >;",
        "    type EquivocationReportSystem = incident_recovery_consensus::RecoveryGrandpaReports;")
    source = replace_once(source,
        "    type EquivocationReportSystem = pallet_babe::EquivocationReportSystem<\n"
        "        Self,\n        liveness::EquivocationReports,\n"
        "        incident_recovery_consensus::RecoveryOwnership,\n        ReportLongevity,\n    >;",
        "    type EquivocationReportSystem = incident_recovery_babe_reports::RecoveryBabeReports;")
    source = replace_once(source, "            _set_id: fg_primitives::SetId,", "            set_id: fg_primitives::SetId,")
    source = replace_once(source, "Historical::prove((fg_primitives::KEY_TYPE, authority_id))", "incident_recovery_consensus::prove_grandpa(set_id, authority_id)")
    source = replace_once(source, "_slot_number: sp_consensus_babe::Slot,", "slot_number: sp_consensus_babe::Slot,")
    source = replace_once(source, "Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))", "incident_recovery_consensus::prove_babe(slot_number, authority_id)")
    source = replace_once(source, "type DisabledValidators = Session;", "type DisabledValidators = incident_recovery_consensus::RecoveryDisabledValidators;")
    source = replace_once(source, "type MessageDispatch = Dispatch;", "type MessageDispatch = incident_recovery_incoming::RecoveryGenericMessageDispatch;")
    source = replace_once(source, "type MessageDispatch = SubstrateDispatch;", "type MessageDispatch = incident_recovery_incoming::RecoverySubstrateMessageDispatch;")
    source = replace_once(source, "    type BridgeAssetLockChecker = BridgeProxy;", "    type BridgeAssetLockChecker = BridgeProxy;\n    type IncomingAccountResolver = incident_recovery_incoming::PersistentAccountResolver;")
    source = replace_once(source, "            EthBridge::get_registered_assets(network_id)\n        }", """            EthBridge::get_registered_assets(network_id)
        }

        fn recovery_hashi_prepare(
            parent_header: Vec<u8>, operation: Vec<u8>,
        ) -> Result<eth_bridge_runtime_api::recovery::RecoveryHashiPreparation, DispatchError> {
            incident_recovery_incoming::witness::prepare(parent_header, operation)
        }

        fn recovery_hashi_admit(
            parent_header: Vec<u8>, operation: Vec<u8>,
        ) -> Result<eth_bridge_runtime_api::recovery::RecoveryHashiAdmission, DispatchError> {
            incident_recovery_incoming::witness::admit(parent_header, operation)
        }

        fn recovery_hashi_prepare_source(
            parent_header: Vec<u8>, operation: Vec<u8>, source: Vec<u8>,
        ) -> Result<eth_bridge_runtime_api::recovery::RecoveryHashiSourcePreparation, DispatchError> {
            incident_recovery_incoming::witness::source::prepare_source(parent_header, operation, source)
        }""")
    if spec == 134:
        source += '\n#[cfg(test)]\nmod incident_recovery_hashi_legacy_tests;\n'
    if spec != 134:
        source = re.sub(r"spec_version:\s*134,", f"spec_version: {spec},", source, count=1)
        source = re.sub(r"transaction_version:\s*131,", "transaction_version: 132,", source, count=1)
    return source

def patch_bridge_api_manifest(source: str) -> str:
    # Keep the deployed SDK references. Only the response-type metadata derive
    # dependency is added; its existing locked version is reused by Cargo.
    source = replace_once(source, "[dependencies]", '[dependencies]\nscale-info = { version = "2", default-features = false, features = ["derive"] }')
    return replace_once(source, "std = [", 'std = [\n    "scale-info/std",')

def patch_consensus(destination: Path) -> None:
    for pallet, anchor in [("babe", "mod randomness;"), ("grandpa", "pub mod migrations;")]:
        folder = destination / "vendor/polkadot-sdk/frame" / pallet / "src"
        source = (folder / "lib.rs").read_text()
        source = replace_once(source, anchor, anchor + "\npub mod recovery;")
        if pallet == "babe":
            source = replace_once(source,
                "\tpub(crate) fn session_index_for_epoch(epoch_index: u64) -> SessionIndex {",
                "\tpub(crate) fn session_index_for_epoch(epoch_index: u64) -> SessionIndex {\n\t\tif let Some(session) = recovery::session_for_epoch(epoch_index) { return session; }")
        if pallet == "babe":
            source = replace_once(source,
                "\t\tif current_epoch_index.saturating_add(1) != epoch_index {",
                "\t\tif current_epoch_index.saturating_add(1) != epoch_index || recovery::resume_anchor_needed(epoch_index, session_index) {")
        (folder / "lib.rs").write_text(source)
        (folder / "recovery.rs").write_bytes((ROOT / "vendor/polkadot-sdk/frame" / pallet / "src/recovery.rs").read_bytes())
    path = destination / "vendor/polkadot-sdk/frame/babe/src/equivocation.rs"
    source = path.read_text()
    source = replace_once(source, "\t\tif Pallet::<T>::session_index_for_epoch(epoch_index) != session_index {",
        "\t\tif Pallet::<T>::session_index_for_epoch(epoch_index) != session_index || !crate::recovery::key_allowed(epoch_index, &offender, validator_set_count) {")
    path.write_text(source)
    path = destination / "vendor/polkadot-sdk/frame/grandpa/src/equivocation.rs"
    source = path.read_text()
    source = replace_once(source, "\t\tlet validator_set_count = key_owner_proof.validator_count();",
        "\t\tlet validator_set_count = key_owner_proof.validator_count();\n\t\tlet recovery_range = crate::recovery::override_range(set_id, session_index, &offender);")
    source = replace_once(source,
        "\t\tif session_index > set_id_session_index ||\n\t\t\tprevious_set_id_session_index\n\t\t\t\t.map(|previous_index| session_index <= previous_index)\n\t\t\t\t.unwrap_or(false)",
        "\t\tif recovery_range == Some(false) || (recovery_range.is_none() && (session_index > set_id_session_index ||\n\t\t\tprevious_set_id_session_index\n\t\t\t\t.map(|previous_index| session_index <= previous_index)\n\t\t\t\t.unwrap_or(false)))")
    path.write_text(source)

def byte_literal(data: bytes) -> str:
    return "[" + ",".join(str(value) for value in data) + "]"

def bind_inputs(path: Path, spec: int) -> tuple[str, dict, bytes | None]:
    config = json.loads(path.read_text())
    required = {"policyScale", "policyCommitment", "operatorsScale", "previousGrandpaSetId", "firstRecoverySlot", "finalWasm"}
    if spec == 134:
        required.add("holdWasm")
    if spec in (136, 137):
        required.update({"originalPolicyScale", "originalPolicyCommitment"})
    if set(config) != required:
        raise ValueError(f"binding fields must be exactly {sorted(required)}")
    policy = bytes.fromhex(config["policyScale"].removeprefix("0x"))
    operators = bytes.fromhex(config["operatorsScale"].removeprefix("0x"))
    commitment = bytes.fromhex(config["policyCommitment"].removeprefix("0x"))
    if not policy or not operators or len(commitment) != 32 or commitment == bytes(32) or hashlib.blake2b(policy, digest_size=32).digest() != commitment:
        raise ValueError("empty or mismatched policy/operator binding")
    original = bytes.fromhex(config.get("originalPolicyScale", config["policyScale"]).removeprefix("0x"))
    original_commitment = bytes.fromhex(config.get("originalPolicyCommitment", config["policyCommitment"]).removeprefix("0x"))
    if not original or len(original_commitment) != 32 or hashlib.blake2b(original, digest_size=32).digest() != original_commitment:
        raise ValueError("original policy hash mismatch")
    # Parsing happens before reading artifact paths. Old policies have no safe
    # implicit domain upgrade: retain their artifacts, then explicitly rebuild.
    if str(HERE.parent) not in sys.path:
        sys.path.insert(0, str(HERE.parent))
    from release_package import policy_identity
    policy_identity_value, original_identity = policy_identity(policy), policy_identity(original)
    if policy_identity_value["authorizationDomain"] != original_identity["authorizationDomain"]:
        raise ValueError("repair cannot change the original recovery authorization domain")
    previous, slot = config["previousGrandpaSetId"], config["firstRecoverySlot"]
    if type(previous) is not int or not 0 <= previous < 2**64-1 or type(slot) is not int or not 0 < slot < 2**64:
        raise ValueError("invalid GRANDPA set or first recovery slot")
    final = (path.parent / config["finalWasm"]).resolve().read_bytes()
    hold = (path.parent / config["holdWasm"]).resolve().read_bytes() if spec == 134 else None
    if not final or len(final) > 32*1024*1024 or (hold is not None and (not hold or len(hold) > 32*1024*1024)):
        raise ValueError("Wasm artifact length out of bounds")
    lines = ["//! Public immutable release inputs; never generated from production secrets.",
        f"pub const POLICY_SCALE: &[u8] = &{byte_literal(policy)};",
        f"pub const POLICY_COMMITMENT: [u8; 32] = {byte_literal(commitment)};",
        f"pub const ORIGINAL_POLICY_SCALE: &[u8] = &{byte_literal(original)};",
        f"pub const ORIGINAL_POLICY_COMMITMENT: [u8; 32] = {byte_literal(original_commitment)};",
        f"pub const OPERATORS_SCALE: &[u8] = &{byte_literal(operators)};",
        f"pub const PREVIOUS_GRANDPA_SET: u64 = {previous};", f"pub const FIRST_RECOVERY_SLOT: u64 = {slot};"]
    for name, blob in [("FINAL", final), ("HOLD", hold or b"")]:
        hash_bytes = hashlib.blake2b(blob, digest_size=32).digest() if blob else bytes(32)
        chunks = [hashlib.blake2b(blob[i:i+65536], digest_size=32).digest() for i in range(0, len(blob), 65536)]
        lines.extend([f"pub const {name}_CODE_HASH: [u8; 32] = {byte_literal(hash_bytes)};", f"pub const {name}_CODE_LEN: u32 = {len(blob)};",
            f"pub const {name}_CHUNK_HASHES: &[[u8; 32]] = &[" + ",".join(byte_literal(chunk) for chunk in chunks) + "];"])
    lines.append('pub const HOLD_WASM: &[u8] = include_bytes!("hold.wasm");' if hold else 'pub const HOLD_WASM: &[u8] = &[];')
    evidence = {"binding": digest(path.read_bytes()), "policy": digest(policy), "operators": digest(operators), "finalWasm": digest(final), "holdWasm": digest(hold) if hold else None}
    return "\n".join(lines) + "\n", evidence, hold

def source_inventory(destination: Path) -> dict:
    """Hash source inputs only; provenance and warm root target are excluded."""
    source_files = {}
    for entry in SOURCE_PATHS:
        root = destination / entry
        for item in (root.rglob("*") if root.is_dir() else [root]):
            if item.is_symlink():
                raise ValueError("source export contains a symlink")
            if item.is_file():
                source_files[item.relative_to(destination).as_posix()] = digest(item.read_bytes())
    return dict(sorted(source_files.items()))

def prepare(destination: Path, spec: int, binding: Path | None) -> dict:
    if destination.exists():
        raise ValueError("destination must be new; existing build lanes are never overwritten")
    kind = subprocess.check_output(["git", "cat-file", "-t", LEGACY_TREE], cwd=ROOT, text=True).strip()
    if kind != "tree":
        raise ValueError("pinned deployed source tree unavailable")
    archive = subprocess.check_output(["git", "archive", "--format=tar", LEGACY_TREE, *SOURCE_PATHS], cwd=ROOT)
    destination.mkdir(parents=True)
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for member in tar.getmembers():
            path = Path(member.name)
            if path.is_absolute() or ".." in path.parts or member.issym() or member.islnk() or member.isdev():
                raise ValueError(f"unexpected source archive entry {member.name}")
        tar.extractall(destination, filter="data")
    base_lock = digest((destination / "Cargo.lock").read_bytes())
    lib = destination / "runtime/src/lib.rs"
    lib.write_text(patch_runtime(lib.read_text(), spec))
    patch_consensus(destination)
    patch = HERE / "bridge-recovery.patch"
    subprocess.run(["git", "apply", "--check", str(patch)], cwd=destination, check=True)
    subprocess.run(["git", "apply", str(patch)], cwd=destination, check=True)
    common_lib = destination / "common/src/lib.rs"
    common_lib.write_text(common_lib.read_text() + "\npub mod recovery;\n")
    (destination / "common/src/recovery.rs").write_bytes((ROOT / "common/src/recovery.rs").read_bytes())
    for source in (ROOT / "common/src/recovery").rglob("*.rs"):
        target = destination / source.relative_to(ROOT)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source.read_bytes())
    manifest = destination / "runtime/Cargo.toml"
    text = manifest.read_text()
    text = replace_once(text, "[dependencies]", '[dependencies]\nsp-trie = { git = "https://github.com/paritytech/polkadot-sdk.git", tag = "polkadot-stable2603-3", default-features = false }')
    text = replace_once(text, "std = [", 'std = [\n    "sp-trie/std",')
    text = replace_once(text, "[dev-dependencies]", '[dev-dependencies]\nsp-externalities = { git = "https://github.com/paritytech/polkadot-sdk.git", tag = "polkadot-stable2603-3" }')
    text = replace_once(text, 'rlp = "0.5.2"\n', "")
    text = replace_once(text, "[dependencies]", '[dependencies]\nrlp = { version = "0.5.2", default-features = false }')
    text = replace_once(text, "std = [", 'std = [\n    "rlp/std",')
    manifest.write_text(text)
    bridge_api_manifest = destination / "pallets/eth-bridge/runtime-api/Cargo.toml"
    bridge_api_manifest.write_text(patch_bridge_api_manifest(bridge_api_manifest.read_text()))
    for source, relative in [(HERE / "overlay/incident_recovery.rs", "runtime/src/incident_recovery.rs"),
        (ROOT / "pallets/eth-bridge/runtime-api/src/lib.rs", "pallets/eth-bridge/runtime-api/src/lib.rs"),
        (ROOT / "pallets/eth-bridge/runtime-api/src/recovery.rs", "pallets/eth-bridge/runtime-api/src/recovery.rs"),
        (ROOT / "runtime/src/incident_recovery_consensus.rs", "runtime/src/incident_recovery_consensus.rs"),
        (ROOT / "runtime/src/incident_recovery_babe_reports.rs", "runtime/src/incident_recovery_babe_reports.rs"),
        (ROOT / "runtime/src/incident_recovery_evm_source.rs", "runtime/src/incident_recovery_evm_source.rs"),
        (ROOT / "runtime/src/incident_recovery_evm_source_tests.rs", "runtime/src/incident_recovery_evm_source_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming.rs", "runtime/src/incident_recovery_incoming.rs"),
        (ROOT / "runtime/src/incident_recovery_hashi_witness.rs", "runtime/src/incident_recovery_hashi_witness.rs"),
        (ROOT / "runtime/src/incident_recovery_hashi_source.rs", "runtime/src/incident_recovery_hashi_source.rs"),
        (ROOT / "runtime/src/incident_recovery_hashi_source_tests.rs", "runtime/src/incident_recovery_hashi_source_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_hashi_witness_tests.rs", "runtime/src/incident_recovery_hashi_witness_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_hashi_legacy_tests.rs", "runtime/src/incident_recovery_hashi_legacy_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming_audit.rs", "runtime/src/incident_recovery_incoming_audit.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming_balance.rs", "runtime/src/incident_recovery_incoming_balance.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming_issuance.rs", "runtime/src/incident_recovery_incoming_issuance.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming_balance_tests.rs", "runtime/src/incident_recovery_incoming_balance_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_hashi_custody_tests.rs", "runtime/src/incident_recovery_hashi_custody_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming_audit_tests.rs", "runtime/src/incident_recovery_incoming_audit_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming_hashi_source_tests.rs", "runtime/src/incident_recovery_incoming_hashi_source_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming_channel_tests.rs", "runtime/src/incident_recovery_incoming_channel_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming_post_complete_channel_tests.rs", "runtime/src/incident_recovery_incoming_post_complete_channel_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_incoming_hashi_delayed_tests.rs", "runtime/src/incident_recovery_incoming_hashi_delayed_tests.rs"),
        (ROOT / "runtime/src/incident_recovery_outgoing.rs", "runtime/src/incident_recovery_outgoing.rs")]:
        (destination / relative).write_bytes(source.read_bytes())
    generated = destination / "runtime/src/incident_recovery/generated.rs"
    generated.parent.mkdir(exist_ok=True)
    if binding:
        code, evidence, hold = bind_inputs(binding.resolve(), spec)
        generated.write_text(code)
        if hold:
            (generated.parent / "hold.wasm").write_bytes(hold)
    else:
        generated.write_bytes((HERE / "overlay/generated.rs").read_bytes())
        evidence = None
    # Record every overlay file, including source files shared with runtime 138.
    overlays = ["runtime/src/lib.rs", "runtime/src/incident_recovery.rs", "runtime/src/incident_recovery_consensus.rs", "runtime/src/incident_recovery_babe_reports.rs",
        "runtime/src/incident_recovery/generated.rs", "runtime/Cargo.toml", "common/src/lib.rs", "common/src/recovery.rs",
        "pallets/eth-bridge/runtime-api/Cargo.toml", "pallets/eth-bridge/runtime-api/src/lib.rs", "pallets/eth-bridge/runtime-api/src/recovery.rs",
        "runtime/src/incident_recovery_hashi_witness.rs", "runtime/src/incident_recovery_hashi_witness_tests.rs", "runtime/src/incident_recovery_hashi_legacy_tests.rs",
        "runtime/src/incident_recovery_hashi_source.rs", "runtime/src/incident_recovery_hashi_source_tests.rs"]
    overlays += [str(path.relative_to(destination)) for path in (destination / "common/src/recovery").rglob("*.rs")]
    overlays += [f"vendor/polkadot-sdk/frame/{pallet}/src/{file}" for pallet in ["babe", "grandpa"] for file in ["lib.rs", "recovery.rs"]]
    overlays.append("vendor/polkadot-sdk/frame/grandpa/src/equivocation.rs")
    overlays.append("vendor/polkadot-sdk/frame/babe/src/equivocation.rs")
    overlays.append("runtime/src/incident_recovery_evm_source.rs")
    overlays.append("runtime/src/incident_recovery_evm_source_tests.rs")
    overlays.append("runtime/src/incident_recovery_incoming.rs")
    overlays.append("runtime/src/incident_recovery_incoming_audit.rs")
    overlays.append("runtime/src/incident_recovery_incoming_balance.rs")
    overlays.append("runtime/src/incident_recovery_incoming_issuance.rs")
    overlays.append("runtime/src/incident_recovery_incoming_balance_tests.rs")
    overlays.append("runtime/src/incident_recovery_hashi_custody_tests.rs")
    overlays.append("runtime/src/incident_recovery_incoming_audit_tests.rs")
    overlays.append("runtime/src/incident_recovery_incoming_hashi_source_tests.rs")
    overlays.append("runtime/src/incident_recovery_incoming_channel_tests.rs")
    overlays.append("runtime/src/incident_recovery_incoming_post_complete_channel_tests.rs")
    overlays.append("runtime/src/incident_recovery_incoming_hashi_delayed_tests.rs")
    overlays.append("runtime/src/incident_recovery_outgoing.rs")
    overlays += re.findall(r"^diff --git a/\S+ b/(\S+)$", patch.read_text(), flags=re.MULTILINE)
    provenance = {"formatVersion": 1, "deployedSourceTree": LEGACY_TREE, "sdk": "polkadot-stable2603-3",
        "specVersion": spec, "transactionVersion": 131 if spec == 134 else 132, "bound": binding is not None, "fixtureOnly": False,
        "sourceFiles": source_inventory(destination), "baseCargoLock": base_lock, "preparer": digest(Path(__file__).read_bytes()), "bridgeOverlay": digest(patch.read_bytes()), "bindingEvidence": evidence,
        "overlay": {name: digest((destination / name).read_bytes()) for name in sorted(overlays)},
        "releaseReady": False, "requiredGates": ["identical historical replay state roots", "preserved call indices plus code-identity metadata admission",
        "full and warp consensus boundary", "all recovery and escape rehearsals", "typed complete state coverage", "exact artifact checksums"]}
    if binding:
        fixture_marker = binding.resolve().parent / "FIXTURE-PROVENANCE.json"
        if fixture_marker.exists():
            marker = json.loads(fixture_marker.read_text())
            if marker.get("fixtureOnly") is not True:
                raise ValueError("invalid adjacent fixture provenance marker")
            provenance["fixtureOnly"] = True
            provenance["fixtureInputs"] = digest(fixture_marker.read_bytes())
            (destination / "NEVER-DEPLOY.txt").write_text("SORA Nexus PUBLIC FIXTURE ONLY — NEVER DEPLOY\n")
    (destination / "RECOVERY-PROVENANCE.json").write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n")
    return provenance

def refresh(destination: Path) -> dict:
    provenance_path = destination / "RECOVERY-PROVENANCE.json"
    previous = json.loads(provenance_path.read_text())
    if previous["bound"] or previous["deployedSourceTree"] != LEGACY_TREE:
        raise ValueError("only an unbound development export can be refreshed")
    for name, expected in previous["overlay"].items():
        if digest((destination / name).read_bytes()) != expected:
            raise ValueError(f"development overlay was edited locally; refusing overwrite: {name}")
    with tempfile.TemporaryDirectory(prefix="soranx-overlay-refresh-") as temporary:
        fresh = Path(temporary) / "source"
        updated = prepare(fresh, previous["specVersion"], None)
        for name in updated["overlay"]:
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            content = (fresh / name).read_bytes()
            # Keep unchanged source mtimes in the warm development lane. Cargo
            # otherwise rebuilds the entire legacy graph after a test-only fix.
            if not target.exists() or target.read_bytes() != content:
                target.write_bytes(content)
        # The refreshed unbound development source keeps its actual lock and
        # untouched files; do not certify the temporary fresh export instead.
        updated["sourceFiles"] = source_inventory(destination)
        updated["refresh"] = {"previousProvenance": digest(provenance_path.read_bytes()),
            "preservedCargoLock": digest((destination / "Cargo.lock").read_bytes()),
            "preservedBuildTarget": True}
        provenance_path.write_text(json.dumps(updated, indent=2, sort_keys=True) + "\n")
    return updated

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--spec-version", type=int, choices=[134, 135, 136, 137], default=134)
    parser.add_argument("--refresh-overlay", action="store_true", help="refresh an untouched unbound export; preserve target and Cargo.lock")
    parser.add_argument("--binding", type=Path, help="reviewed public release binding JSON; omit for fail-closed development export")
    args = parser.parse_args()
    try:
        if args.refresh_overlay and args.binding:
            raise ValueError("cannot refresh an immutable bound export")
        result = refresh(args.destination.resolve()) if args.refresh_overlay else prepare(args.destination.resolve(), args.spec_version, args.binding)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"recovery export rejected: {error}\n")
    # Bind stdout to the exact in-process provenance value, not a subsequent
    # rehash of a mutable file after the export completed.
    provenance_bytes = (json.dumps(result, indent=2, sort_keys=True) + "\n").encode()
    print(json.dumps({"destination": str(args.destination.resolve()), "bound": result["bound"],
        "provenancePin": {**digest(provenance_bytes), "bytes": len(provenance_bytes)}, "releaseReady": False}))

if __name__ == "__main__":
    main()
