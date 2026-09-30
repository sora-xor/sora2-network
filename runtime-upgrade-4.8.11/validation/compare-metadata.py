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
        kind = next(iter(old_def))
        if not self.same(kind, next(iter(new_def)), path + ".kind"):
            return
        left, right = old_def[kind], new_def[kind]
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
            for key in ("bitStoreType", "bitOrderType"):
                self.type(left[key], right[key], path + "." + key, seen)
            # The order marker may be a zero-field type whose identity matters.
            self.same(self.types[0][left["bitOrderType"]].get("path"),
                      self.types[1][right["bitOrderType"]].get("path"), path + ".bitOrderPath")
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
        elif kind == "bitSequence":
            for key in ("bitStoreType", "bitOrderType"): value[key] += shift
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
    return checks


def main():
    here = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old", type=Path, default=here / "reference-runtime-130/framenode-runtime-4.8.8-metadata.json")
    parser.add_argument("--new", type=Path, default=here.parent / "framenode-runtime-4.8.9-metadata.json")
    parser.add_argument("--output", type=Path, default=here / "metadata-compatibility.json")
    args = parser.parse_args()
    old, new = load(args.old), load(args.new)
    result = Comparison(old, new).run()
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
