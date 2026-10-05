#!/usr/bin/env python3
"""Compare V14 subwasm JSON structurally; portable type IDs are not identities.

Existing SCALE values must retain their encoding. Adding enum variants at new
indices is compatible with old values, but is reported separately. Metadata does
not prove runtime behavior, migration correctness, or custom codec equivalence.
"""

import argparse
import copy
import hashlib
import json
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path):
    data = json.loads(path.read_text())
    if set(data) != {"V14"}:
        raise ValueError(f"Expected subwasm V14 metadata: {path}")
    return data["V14"]


def indexed(items, key):
    result = {item[key]: item for item in items}
    if len(result) != len(items):
        raise ValueError(f"Duplicate {key} in metadata")
    return result


def bit_sequence_refs(definition):
    # subwasm's Rust serde JSON uses snake_case; some metadata producers use
    # scale-info's camelCase representation. Both reference portable type IDs.
    for store_key, order_key in (("bit_store_type", "bit_order_type"),
                                 ("bitStoreType", "bitOrderType")):
        if set(definition) == {store_key, order_key}:
            return definition[store_key], definition[order_key]
    raise ValueError("Malformed bit-sequence definition")


SPONSORSHIP_REMOVALS = {
    "calls": {1: "sponsor_migration", 2: "revoke_sponsorship"},
    "event": {1: "FeeSponsorshipGranted", 2: "FeeSponsorshipUsed", 3: "FeeSponsorshipRevoked"},
    "error": {10: "InvalidMigrationInput", 11: "InvalidFeeSponsorship", 12: "NotFeeSponsor"},
}


def without_undeployed_sponsorship(old, new):
    """Project only the explicitly withdrawn, never-deployed Iroha sponsorship ABI.

    Canonical enum IDs are shared by nested RuntimeCall/RuntimeEvent references,
    so editing these exact enum definitions also handles recursive references.
    All remaining types, variants, constants and signed extensions are compared
    by the unchanged strict comparator, rather than filtering its failures.
    """
    projected = copy.deepcopy(old)
    before = next(p for p in projected["pallets"] if p["name"] == "IrohaMigration")
    after = next(p for p in new["pallets"] if p["name"] == "IrohaMigration")
    old_types = {item["id"]: item["type"] for item in projected["types"]["types"]}
    new_types = {item["id"]: item["type"] for item in new["types"]["types"]}
    removed = []
    for category, expected in SPONSORSHIP_REMOVALS.items():
        variants = old_types[before[category]["ty"]]["def"]["variant"]["variants"]
        replacements = new_types[after[category]["ty"]]["def"]["variant"]["variants"]
        by_index = indexed(variants, "index")
        for index, name in expected.items():
            if index not in by_index or by_index[index]["name"] != name:
                raise ValueError("Predecessor lacks exact undeployed sponsor variant: " + category + "." + name)
            if any(item["index"] == index or item["name"] == name for item in replacements):
                raise ValueError("Withdrawn sponsor variant remains or its index was reused: " + name)
            removed.append({"path": "IrohaMigration." + category, "variant": name, "variantIndex": index})
        old_types[before[category]["ty"]]["def"]["variant"]["variants"] = [
            item for item in variants if item["index"] not in expected]
    migrate = new_types[after["calls"]["ty"]]["def"]["variant"]["variants"]
    if not any(item["name"] == "migrate" and item["index"] == 0 for item in migrate):
        raise ValueError("Legacy migrate call must remain at index zero")
    entries = before["storage"]["entries"]
    if sum(item["name"] == "FeeSponsorships" for item in entries) != 1:
        raise ValueError("Predecessor must contain exactly one FeeSponsorships entry")
    if any(item["name"] == "FeeSponsorships" for item in after["storage"]["entries"]):
        raise ValueError("Withdrawn FeeSponsorships storage remains")
    before["storage"]["entries"] = [item for item in entries if item["name"] != "FeeSponsorships"]
    removed.append({"path": "IrohaMigration.storage.FeeSponsorships", "storage": "FeeSponsorships"})
    return projected, removed


def sponsorship_removal_comparison(old, new):
    projected, removed = without_undeployed_sponsorship(old, new)
    result = Comparison(projected, new).run()
    result["allowedUndeployedSponsorshipRemovals"] = removed
    exact = result["compatibleExistingScaleEncoding"] and not result["additions"] and not result["constantValueChanges"]
    result["compatibleExistingScaleEncoding"] = exact
    result["legacyAbiPreservedExceptDeclaredUndeployedSponsorship"] = exact
    result["allOtherAbiAndConstantsPreserved"] = exact
    return result


class Comparison:
    def __init__(self, old, new):
        self.old, self.new = old, new
        self.types = [
            {item["id"]: item["type"] for item in data["types"]["types"]}
            for data in (old, new)
        ]
        self.breaks = []
        self.additions = []
        self.changes = []
        self.counts = Counter()

    def same(self, old, new, path):
        if old != new:
            self.breaks.append({"path": path, "old": old, "new": new})
            return False
        return True

    def fields(self, old, new, path, seen):
        self.same(len(old), len(new), path + ".length")
        for index, (left, right) in enumerate(zip(old, new)):
            at = f"{path}[{index}]"
            self.same(left.get("name"), right.get("name"), at + ".name")
            self.type(left["type"], right["type"], at + ".type", seen)

    def type(self, old_id, new_id, path, seen=None):
        # Per-root pair traversal checks recursive graphs without assuming the
        # same ID, path name, or generic parameter numbering across runtimes.
        seen = set() if seen is None else seen
        pair = (old_id, new_id)
        if pair in seen:
            return
        seen.add(pair)
        self.counts["nestedTypePairsVisited"] += 1
        old, new = self.types[0][old_id], self.types[1][new_id]
        old_def, new_def = old["def"], new["def"]
        if len(old_def) != 1 or len(new_def) != 1:
            raise ValueError("Malformed type definition")
        old_kind, new_kind = next(iter(old_def)), next(iter(new_def))
        normalize_kind = lambda name: "bitSequence" if name == "bitsequence" else name
        kind = normalize_kind(old_kind)
        if not self.same(kind, normalize_kind(new_kind), path + ".kind"):
            return
        left, right = old_def[old_kind], new_def[new_kind]
        if kind == "primitive":
            self.same(left, right, path + ".primitive")
        elif kind in ("sequence", "compact", "array"):
            if kind == "array":
                self.same(left["len"], right["len"], path + ".array.length")
            self.type(left["type"], right["type"], path + ".element", seen)
        elif kind == "composite":
            self.fields(left.get("fields", []), right.get("fields", []), path + ".fields", seen)
        elif kind == "tuple":
            self.same(len(left), len(right), path + ".tuple.length")
            for index, (a, b) in enumerate(zip(left, right)):
                self.type(a, b, f"{path}.tuple[{index}]", seen)
        elif kind == "variant":
            old_variants = indexed(left.get("variants", []), "index")
            new_variants = indexed(right.get("variants", []), "index")
            for index, variant in old_variants.items():
                at = f"{path}.variant[{index}]"
                if index not in new_variants:
                    self.breaks.append({"path": at, "removedVariant": variant["name"]})
                    continue
                replacement = new_variants[index]
                self.same(variant["name"], replacement["name"], at + ".name")
                self.fields(variant.get("fields", []), replacement.get("fields", []), at + ".fields", seen)
            for index in new_variants.keys() - old_variants.keys():
                self.additions.append({"path": path, "variantIndex": index, "variant": new_variants[index]["name"]})
        elif kind == "bitSequence":
            old_store, old_order = bit_sequence_refs(left)
            new_store, new_order = bit_sequence_refs(right)
            self.type(old_store, new_store, path + ".bitStoreType", seen)
            self.type(old_order, new_order, path + ".bitOrderType", seen)
            # The order marker may be a zero-field type whose identity matters.
            old_path, new_path = self.types[0][old_order].get("path"), self.types[1][new_order].get("path")
            if not old_path or not new_path:
                raise ValueError("Bit order marker must have an identity path")
            self.same(old_path, new_path, path + ".bitOrderPath")
        else:
            raise ValueError(f"Unsupported portable type kind: {kind}")

    def storage(self, old, new, path):
        if not old:
            if new:
                self.additions.append({"path": path, "storage": new})
            return
        if not new:
            self.breaks.append({"path": path, "removedStorage": True})
            return
        self.same(old["prefix"], new["prefix"], path + ".prefix")
        old_entries = indexed(old["entries"], "name")
        new_entries = indexed(new["entries"], "name")
        for name, entry in old_entries.items():
            at = path + "." + name
            self.counts["existingStorageEntries"] += 1
            if name not in new_entries:
                self.breaks.append({"path": at, "removed": True})
                continue
            other = new_entries[name]
            for field in ("modifier", "default"):
                self.same(entry[field], other[field], at + "." + field)
            kind = next(iter(entry["ty"]))
            if not self.same(kind, next(iter(other["ty"])), at + ".kind"):
                continue
            left, right = entry["ty"][kind], other["ty"][kind]
            if kind == "Plain":
                self.type(left, right, at + ".value")
            elif kind == "Map":
                self.same(left["hashers"], right["hashers"], at + ".hashers")
                self.type(left["key"], right["key"], at + ".key")
                self.type(left["value"], right["value"], at + ".value")
            else:
                raise ValueError(f"Unsupported storage kind: {kind}")
        for name in sorted(new_entries.keys() - old_entries.keys()):
            entry = new_entries[name]
            self.additions.append({"path": path + "." + name, "storage": {
                key: entry[key] for key in ("modifier", "ty", "default")
            }})

    def run(self):
        old_pallets = indexed(self.old["pallets"], "name")
        new_pallets = indexed(self.new["pallets"], "name")
        indexed(self.old["pallets"], "index")
        indexed(self.new["pallets"], "index")
        for name, old in old_pallets.items():
            self.counts["existingPallets"] += 1
            if name not in new_pallets:
                self.breaks.append({"path": name, "removedPallet": True})
                continue
            new = new_pallets[name]
            self.same(old["index"], new["index"], name + ".index")
            for category in ("calls", "event", "error"):
                if old.get(category):
                    if not new.get(category):
                        self.breaks.append({"path": name + "." + category, "removed": True})
                        continue
                    old_id, new_id = old[category]["ty"], new[category]["ty"]
                    self.counts["existing" + category.capitalize()] += len(
                        self.types[0][old_id]["def"]["variant"].get("variants", []))
                    self.type(old_id, new_id, name + "." + category)
                elif new.get(category):
                    self.additions.append({"path": name + "." + category, "added": True})
            self.storage(old.get("storage"), new.get("storage"), name + ".storage")
            old_constants = indexed(old.get("constants", []), "name")
            new_constants = indexed(new.get("constants", []), "name")
            for cname, constant in old_constants.items():
                at = name + ".constants." + cname
                self.counts["existingConstants"] += 1
                if cname not in new_constants:
                    self.breaks.append({"path": at, "removed": True})
                    continue
                other = new_constants[cname]
                self.type(constant["ty"], other["ty"], at + ".type")
                if constant["value"] != other["value"]:
                    self.changes.append({"path": at, "oldValue": constant["value"], "newValue": other["value"]})
            for cname in sorted(new_constants.keys() - old_constants.keys()):
                constant = new_constants[cname]
                self.additions.append({"path": name + ".constants." + cname, "value": constant["value"]})
        for name in sorted(new_pallets.keys() - old_pallets.keys()):
            self.additions.append({"path": name, "palletIndex": new_pallets[name]["index"]})

        old_ext, new_ext = self.old["extrinsic"], self.new["extrinsic"]
        self.same(old_ext["version"], new_ext["version"], "extrinsic.version")
        self.type(old_ext["ty"], new_ext["ty"], "extrinsic.type")
        # UncheckedExtrinsic exposes a byte-vector as its definition; its
        # generic parameters carry the actual address/call/signature/extra.
        old_params = self.types[0][old_ext["ty"]].get("params", [])
        new_params = self.types[1][new_ext["ty"]].get("params", [])
        self.same([x["name"] for x in old_params], [x["name"] for x in new_params], "extrinsic.parameterOrder")
        for old, new in zip(old_params, new_params):
            self.type(old["type"], new["type"], "extrinsic.parameter." + old["name"])
        old_signed, new_signed = old_ext["signed_extensions"], new_ext["signed_extensions"]
        self.same([x["identifier"] for x in old_signed], [x["identifier"] for x in new_signed], "extrinsic.signedExtensionOrder")
        for old, new in zip(old_signed, new_signed):
            self.counts["signedExtensions"] += 1
            for key in ("ty", "additional_signed"):
                self.type(old[key], new[key], "signedExtensions." + old["identifier"] + "." + key)
        # Multiple roots can reach the same added variant; retain each path to
        # show precisely where new values may surface, but deduplicate repeats.
        unique = lambda values: [json.loads(x) for x in sorted({json.dumps(v, sort_keys=True) for v in values})]
        return {"compatibleExistingScaleEncoding": not self.breaks,
                "counts": dict(self.counts), "breakingChanges": unique(self.breaks),
                "additions": unique(self.additions), "constantValueChanges": self.changes,
                "signedExtensionIdentifiers": [x["identifier"] for x in old_signed]}


def self_checks(metadata):
    """Exercise ID churn and defects beneath otherwise unchanged root types."""
    checks = []
    shifted = copy.deepcopy(metadata)
    shift = 10000
    for item in shifted["types"]["types"]:
        item["id"] += shift
        ty = item["type"]
        for param in ty.get("params", []):
            if param.get("type") is not None:
                param["type"] += shift
        kind, value = next(iter(ty["def"].items()))
        if kind in ("sequence", "compact", "array"):
            value["type"] += shift
        elif kind == "composite":
            for field in value.get("fields", []): field["type"] += shift
        elif kind == "variant":
            for variant in value.get("variants", []):
                for field in variant.get("fields", []): field["type"] += shift
        elif kind == "tuple":
            ty["def"][kind] = [x + shift for x in value]
        elif kind in ("bitSequence", "bitsequence"):
            bit_sequence_refs(value)  # Validate the exact schema before rewriting IDs.
            for key in value: value[key] += shift
        elif kind != "primitive":
            raise ValueError(kind)
    for pallet in shifted["pallets"]:
        for key in ("calls", "event", "error"):
            if pallet.get(key): pallet[key]["ty"] += shift
        for constant in pallet.get("constants", []): constant["ty"] += shift
        for entry in (pallet.get("storage") or {}).get("entries", []):
            kind, value = next(iter(entry["ty"].items()))
            if kind == "Plain": entry["ty"][kind] += shift
            else:
                value["key"] += shift
                value["value"] += shift
    shifted["ty"] += shift
    shifted["extrinsic"]["ty"] += shift
    for ext in shifted["extrinsic"]["signed_extensions"]:
        ext["ty"] += shift
        ext["additional_signed"] += shift
    result = Comparison(metadata, shifted).run()
    assert result["compatibleExistingScaleEncoding"] and not result["additions"] and not result["constantValueChanges"]
    checks.append("All portable IDs shifted by10000: unchanged semantics accepted")

    # primitive u32 is nested in the staking call's EraIndex, System.Account
    # nonce and CheckNonce's compact value; each separate root must detect it.
    mutated = copy.deepcopy(metadata)
    primitive = next(x for x in mutated["types"]["types"] if x["type"]["def"] == {"primitive": "u32"})
    primitive["type"]["def"] = {"primitive": "u64"}
    paths = [x["path"] for x in Comparison(metadata, mutated).run()["breakingChanges"]]
    for root in ("Staking.calls", "System.storage.Account", "signedExtensions.CheckNonce"):
        assert any(path.startswith(root) for path in paths), root
    checks.append("Nested u32-to-u64 mutation rejected for staking calls, storage and CheckNonce")

    mutated = copy.deepcopy(metadata)
    types = {x["id"]: x["type"] for x in mutated["types"]["types"]}
    pallet = next(x for x in mutated["pallets"] if x["name"] == "Staking")
    variants = types[pallet["calls"]["ty"]]["def"]["variant"]["variants"]
    variant = next(x for x in variants if x["name"] == "payout_stakers")
    variant["index"] = 255
    assert not Comparison(metadata, mutated).run()["compatibleExistingScaleEncoding"]
    checks.append("Changed payout_stakers call index rejected")

    mutated = copy.deepcopy(metadata)
    mutated["extrinsic"]["signed_extensions"].reverse()
    assert not Comparison(metadata, mutated).run()["compatibleExistingScaleEncoding"]
    checks.append("Signed-extension reorder rejected")

    mutated = copy.deepcopy(metadata)
    entry = next(x for x in mutated["pallets"][0]["storage"]["entries"] if x["name"] == "Account")
    entry["ty"]["Map"]["hashers"] = ["Identity"]
    assert not Comparison(metadata, mutated).run()["compatibleExistingScaleEncoding"]
    checks.append("Storage hasher mutation rejected")
    checks.extend(bit_sequence_self_checks())
    return checks


def bit_sequence_self_checks():
    """Always exercise bit-vector encoding, including on mainnet without BitVec roots."""
    model = {"types": {"types": [
        {"id": 0, "type": {"def": {"primitive": "u8"}}},
        {"id": 1, "type": {"def": {"primitive": "u16"}}},
        {"id": 2, "type": {"path": ["bitvec", "order", "Msb0"], "def": {"composite": {}}}},
        {"id": 3, "type": {"path": ["bitvec", "order", "Lsb0"], "def": {"composite": {}}}},
        {"id": 4, "type": {"def": {"bitsequence": {"bit_store_type": 0, "bit_order_type": 2}}}},
    ]}}

    def compare(other):
        comparison = Comparison(model, other)
        comparison.type(4, 4, "BitVector")
        return comparison.breaks

    assert not compare(copy.deepcopy(model))
    camel_case = copy.deepcopy(model)
    camel_case["types"]["types"][4]["type"]["def"] = {
        "bitSequence": {"bitStoreType": 0, "bitOrderType": 2}}
    assert not compare(camel_case)

    changed_store = copy.deepcopy(model)
    changed_store["types"]["types"][4]["type"]["def"]["bitsequence"]["bit_store_type"] = 1
    assert any(item["path"] == "BitVector.bitStoreType.primitive" for item in compare(changed_store))

    changed_order = copy.deepcopy(model)
    changed_order["types"]["types"][4]["type"]["def"]["bitsequence"]["bit_order_type"] = 3
    assert any(item["path"] == "BitVector.bitOrderPath" for item in compare(changed_order))

    changed_marker_shape = copy.deepcopy(model)
    changed_marker_shape["types"]["types"][2]["type"]["def"] = {
        "composite": {"fields": [{"type": 0}]}}
    assert any(item["path"] == "BitVector.bitOrderType.fields.length"
               for item in compare(changed_marker_shape))
    return [
        "Identical bit-sequence storage/order and equivalent serde spellings accepted",
        "Bit-sequence storage width u8-to-u16 rejected",
        "Zero-field bit-order marker Msb0-to-Lsb0 rejected by identity path",
        "Changed bit-order marker layout rejected despite unchanged identity path",
    ]


def sponsorship_removal_self_checks(metadata):
    trimmed = copy.deepcopy(metadata)
    types = {item["id"]: item["type"] for item in trimmed["types"]["types"]}
    pallet = next(p for p in trimmed["pallets"] if p["name"] == "IrohaMigration")
    for category, expected in SPONSORSHIP_REMOVALS.items():
        variants = types[pallet[category]["ty"]]["def"]["variant"]["variants"]
        types[pallet[category]["ty"]]["def"]["variant"]["variants"] = [item for item in variants if item["index"] not in expected]
    pallet["storage"]["entries"] = [item for item in pallet["storage"]["entries"] if item["name"] != "FeeSponsorships"]
    accepted = sponsorship_removal_comparison(metadata, trimmed)
    assert accepted["compatibleExistingScaleEncoding"] and len(accepted["allowedUndeployedSponsorshipRemovals"]) == 9
    assert not accepted["additions"] and not accepted["constantValueChanges"]
    assert not Comparison(metadata, trimmed).run()["compatibleExistingScaleEncoding"], "Default comparison must remain strict"
    extra = copy.deepcopy(trimmed)
    iroha = next(p for p in extra["pallets"] if p["name"] == "IrohaMigration")
    iroha["storage"]["entries"] = [entry for entry in iroha["storage"]["entries"] if entry["name"] != "Balances"]
    assert not sponsorship_removal_comparison(metadata, extra)["compatibleExistingScaleEncoding"]
    extra = copy.deepcopy(trimmed)
    lookup = {item["id"]: item["type"] for item in extra["types"]["types"]}
    iroha = next(p for p in extra["pallets"] if p["name"] == "IrohaMigration")
    lookup[iroha["calls"]["ty"]]["def"]["variant"]["variants"][0]["fields"][0]["name"] = "changed_legacy_argument"
    assert not sponsorship_removal_comparison(metadata, extra)["compatibleExistingScaleEncoding"]
    extra = copy.deepcopy(trimmed)
    extra["extrinsic"]["signed_extensions"].reverse()
    assert not sponsorship_removal_comparison(metadata, extra)["compatibleExistingScaleEncoding"]
    extra = copy.deepcopy(trimmed)
    lookup = {item["id"]: item["type"] for item in extra["types"]["types"]}
    iroha = next(p for p in extra["pallets"] if p["name"] == "IrohaMigration")
    lookup[iroha["calls"]["ty"]]["def"]["variant"]["variants"].append({"name": "unrequested_call", "index": 42, "fields": []})
    assert not sponsorship_removal_comparison(metadata, extra)["compatibleExistingScaleEncoding"]
    extra = copy.deepcopy(trimmed)
    version = next(item for pallet in extra["pallets"] if pallet["name"] == "System" for item in pallet["constants"] if item["name"] == "Version")
    version["value"][0] ^= 1
    assert not sponsorship_removal_comparison(metadata, extra)["compatibleExistingScaleEncoding"]
    return ["Exact nine undeployed Iroha sponsor removals accepted only in explicit mode",
            "Additional legacy Iroha storage removal rejected",
            "Changed migrate-zero argument rejected under sponsor-removal mode",
            "Signed-extension reorder rejected under sponsor-removal mode",
            "Unrequested ABI addition rejected under sponsor-removal mode",
            "Constant-byte change rejected under sponsor-removal mode"]


def main():
    here = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old", type=Path, default=here / "reference-runtime-130/framenode-runtime-4.8.8-metadata.json")
    parser.add_argument("--new", type=Path, default=here.parent / "framenode-runtime-4.8.9-metadata.json")
    parser.add_argument("--output", type=Path, default=here / "metadata-compatibility.json")
    parser.add_argument("--allow-undeployed-sponsorship-removal", action="store_true")
    args = parser.parse_args()
    old, new = load(args.old), load(args.new)
    result = sponsorship_removal_comparison(old, new) if args.allow_undeployed_sponsorship_removal else Comparison(old, new).run()
    result["checkedAt"] = datetime.now(timezone.utc).isoformat()
    result["metadataVersion"] = 14
    result["inputs"] = {label: {"path": str(path.resolve()), "sha256": sha256(path)}
                        for label, path in (("baselineMetadata", args.old), ("candidateMetadata", args.new), ("comparisonScript", Path(__file__)))}
    baseline_code = args.old.with_name("framenode-runtime-4.8.8.compact.compressed.wasm")
    captured_code = here / "live-runtime-130.compact.compressed.wasm"
    if baseline_code.exists() and captured_code.exists():
        result["baselineCodeMatchesCapturedMainnet"] = sha256(baseline_code) == sha256(captured_code)
        result["baselineCodeSha256"] = sha256(baseline_code)
        result["capturedMainnetCodeSha256"] = sha256(captured_code)
        if (here / "live-chain.json").exists():
            live = json.loads((here / "live-chain.json").read_text())
            result["capturedMainnetBlock"] = {key: live[key] for key in ("blockHash", "blockNumber", "checkedAt")}
    result["selfChecks"] = self_checks(old)
    if args.allow_undeployed_sponsorship_removal:
        result["selfChecks"].extend(sponsorship_removal_self_checks(old))
    result["comparisonMode"] = "exact-undeployed-Iroha-sponsorship-removal" if args.allow_undeployed_sponsorship_removal else "strict"
    result["limitations"] = [
        "Structural metadata comparison verifies declared SCALE layouts, indices, field order/names, storage hashers/defaults and signed payload shapes. It does not execute migrations or verify behavior or custom codec implementations.",
        "New enum variants preserve encoding of old values. Older metadata cannot decode those new variants.",
        "Source paths, documentation, generic type IDs and typeName aliases are ignored; extrinsic Address/Call/Signature/Extra generic parameters are explicitly traversed.",
        "Constant values may intentionally change. Their encoded types are checked and changed bytes are reported separately.",
    ]
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"compatibleExistingScaleEncoding": result["compatibleExistingScaleEncoding"],
                      "counts": result["counts"], "breakingChanges": result["breakingChanges"],
                      "additions": len(result["additions"]), "constantValueChanges": len(result["constantValueChanges"]),
                      "selfChecks": len(result["selfChecks"]), "output": str(args.output)}, indent=2))
    return 0 if result["compatibleExistingScaleEncoding"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
