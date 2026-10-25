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


if not __debug__:
    raise RuntimeError("Metadata verification requires assertions; do not use -O/PYTHONOPTIMIZE")


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


RETIREMENT_ERRORS = {"Kensetsu": 19, "ApolloPlatform": 36}
RETIREMENT_ADDITIONS = [
    {"path": "Kensetsu.constants.RepaymentOnly", "primitive": "bool", "value": [1]},
    {"path": "Kensetsu.error", "variant": "RepaymentOnly", "variantIndex": 19},
    {"path": "ApolloPlatform.constants.RepaymentOnly", "primitive": "bool", "value": [1]},
    {"path": "ApolloPlatform.error", "variant": "RepaymentOnly", "variantIndex": 36},
    {"path": "ApolloPlatform.storage.DeferredProtocolInterest", "modifier": "Default",
     "hashers": ["Identity"], "key": "existing PoolData asset key", "valuePrimitive": "u128",
     "default": [0] * 16},
]


def without_retirement_additions(old, new):
    """Check exact retirement additions, then compare the remaining candidate unchanged.

    The old paid-migration ABI has no sponsorship to remove. This projection only
    strips two bool constants, two empty error variants and one precisely typed
    zero-default asset map from the NEW metadata. All old encodings remain subject
    to the strict structural comparison, including recursively shared enum types.
    """
    projected = copy.deepcopy(new)
    old_pallets = indexed(old["pallets"], "name")
    new_pallets = indexed(projected["pallets"], "name")
    old_types = indexed(old["types"]["types"], "id")
    new_types = indexed(projected["types"]["types"], "id")
    for name, index in RETIREMENT_ERRORS.items():
        before, after = old_pallets[name], new_pallets[name]
        old_constants = indexed(before.get("constants", []), "name")
        constants = indexed(after.get("constants", []), "name")
        if "RepaymentOnly" in old_constants or "RepaymentOnly" not in constants:
            raise ValueError("RepaymentOnly must be a new constant: " + name)
        constant = constants["RepaymentOnly"]
        if new_types[constant["ty"]]["type"]["def"] != {"primitive": "bool"} or constant["value"] != [1]:
            raise ValueError("RepaymentOnly must be bool true: " + name)
        after["constants"] = [value for value in after["constants"] if value["name"] != "RepaymentOnly"]
        previous_errors = old_types[before["error"]["ty"]]["type"]["def"]["variant"]["variants"]
        errors = new_types[after["error"]["ty"]]["type"]["def"]["variant"]["variants"]
        indexed(errors, "index")
        if any(value["index"] == index or value["name"] == "RepaymentOnly" for value in previous_errors):
            raise ValueError("Retirement error must occupy its unused index: " + name)
        matches = [value for value in errors if value["index"] == index or value["name"] == "RepaymentOnly"]
        if len(matches) != 1 or matches[0]["index"] != index or matches[0]["name"] != "RepaymentOnly" or matches[0].get("fields", []):
            raise ValueError("Retirement error must be the exact empty appended variant: " + name)
        new_types[after["error"]["ty"]]["type"]["def"]["variant"]["variants"] = [
            value for value in errors if value["index"] != index]
    before = old_pallets["ApolloPlatform"]
    after = new_pallets["ApolloPlatform"]
    old_entries = indexed(before["storage"]["entries"], "name")
    entries = indexed(after["storage"]["entries"], "name")
    if "DeferredProtocolInterest" in old_entries or "DeferredProtocolInterest" not in entries:
        raise ValueError("DeferredProtocolInterest must be a new storage entry")
    entry = entries["DeferredProtocolInterest"]
    if entry["modifier"] != "Default" or entry["default"] != [0] * 16 or set(entry["ty"]) != {"Map"}:
        raise ValueError("DeferredProtocolInterest must be a zero-default map")
    mapping = entry["ty"]["Map"]
    if set(mapping) != {"hashers", "key", "value"} or mapping["hashers"] != ["Identity"]:
        raise ValueError("DeferredProtocolInterest must use one Identity asset key")
    if new_types[mapping["value"]]["type"]["def"] != {"primitive": "u128"}:
        raise ValueError("DeferredProtocolInterest must store u128 balances")
    key_check = Comparison(old, projected)
    key_check.type(old_entries["PoolData"]["ty"]["Map"]["key"], mapping["key"], "DeferredProtocolInterest.assetKey")
    if key_check.breaks or key_check.additions:
        raise ValueError("DeferredProtocolInterest key differs from the existing Apollo asset key")
    after["storage"]["entries"] = [value for value in after["storage"]["entries"] if value["name"] != "DeferredProtocolInterest"]
    return projected


def retirement_comparison(old, new):
    projected = without_retirement_additions(old, new)
    remaining = Comparison(old, projected).run()
    result = Comparison(old, new).run()
    exact = (remaining["compatibleExistingScaleEncoding"] and not remaining["additions"]
             and not remaining["constantValueChanges"])
    result["compatibleExistingScaleEncoding"] = exact
    result["onlyDeclaredRetirementAdditions"] = exact
    result["allOtherAbiAndConstantsPreserved"] = exact
    result["allowedRetirementAdditions"] = copy.deepcopy(RETIREMENT_ADDITIONS)
    result["unexpectedRemainingAdditions"] = remaining["additions"]
    return result


def sdk2609_comparison(old, new):
    """Project only precisely reviewed upstream SDK2609 interface changes.

    Raw structural breaks remain visible. In particular CodeUpdated gained a
    hash payload and is not binary compatible with the old event. Projection
    checks every other existing encoding rather than claiming universal ABI
    compatibility. The transaction-version bump remains a caller gate.
    """
    raw = Comparison(old, new).run()
    projected = copy.deepcopy(new)
    old_signed = old["extrinsic"]["signed_extensions"]
    new_signed = projected["extrinsic"]["signed_extensions"]
    expected = ["AuthorizeCall"] + [entry["identifier"] for entry in old_signed]
    if [entry["identifier"] for entry in new_signed] != expected:
        raise ValueError("SDK2609 requires only a leading AuthorizeCall extension")
    types = indexed(projected["types"]["types"], "id")
    authorize = new_signed[0]
    definition = types[authorize["ty"]]["type"]
    if definition.get("path") != ["frame_system", "extensions", "authorize_call", "AuthorizeCall"]:
        raise ValueError("AuthorizeCall must retain its reviewed SDK identity")
    if definition["def"] not in ({"composite": {}}, {"composite": {"fields": []}}):
        raise ValueError("AuthorizeCall must encode zero explicit bytes")
    if types[authorize["additional_signed"]]["type"]["def"] != {"tuple": []}:
        raise ValueError("AuthorizeCall must have unit implicit signing data")
    extrinsic_type = types[projected["extrinsic"]["ty"]]["type"]
    extras = [param for param in extrinsic_type["params"] if param["name"] == "Extra"]
    if len(extras) != 1:
        raise ValueError("Expected one extrinsic Extra type")
    extra_type = types[extras[0]["type"]]["type"]
    if set(extra_type["def"]) != {"tuple"}:
        raise ValueError("SDK2609 Extra must be a tuple")
    if len(extra_type["def"]["tuple"]) != len(old_signed) + 1 or extra_type["def"]["tuple"][0] != authorize["ty"]:
        raise ValueError("Extra tuple must begin with the exact AuthorizeCall type")
    extra_type["def"]["tuple"] = extra_type["def"]["tuple"][1:]
    projected["extrinsic"]["signed_extensions"] = new_signed[1:]
    old_types = indexed(old["types"]["types"], "id")
    old_pallets = indexed(old["pallets"], "name")
    new_pallets = indexed(projected["pallets"], "name")

    def event(model_types, pallets, pallet, name):
        variants = model_types[pallets[pallet]["event"]["ty"]]["type"]["def"]["variant"]["variants"]
        matches = [value for value in variants if value["name"] == name]
        if len(matches) != 1:
            raise ValueError("Expected one event " + pallet + "." + name)
        return matches[0]

    before = event(old_types, old_pallets, "System", "CodeUpdated")
    after = event(types, new_pallets, "System", "CodeUpdated")
    fields = after.get("fields", [])
    if before["index"] != 2 or after["index"] != 2 or before.get("fields", []) or len(fields) != 1 or fields[0].get("name") != "hash":
        raise ValueError("System.CodeUpdated must add only its reviewed hash field at index2")
    hash_type = next(ext["additional_signed"] for ext in old_signed if ext["identifier"] == "CheckGenesis")
    hash_check = Comparison(old, projected)
    hash_check.type(hash_type, fields[0]["type"], "System.CodeUpdated.hash")
    if hash_check.breaks or hash_check.additions:
        raise ValueError("CodeUpdated hash must preserve the existing runtime hash encoding")
    after["fields"] = []

    before = event(old_types, old_pallets, "Offences", "Offence")
    after = event(types, new_pallets, "Offences", "Offence")
    if before["index"] != 0 or after["index"] != 0 or [field.get("name") for field in before["fields"]] != ["kind", "timeslot"] or [field.get("name") for field in after["fields"]] != ["kind", "slot"]:
        raise ValueError("Offences.Offence must only rename its second field timeslot to slot")
    after["fields"][1]["name"] = "timeslot"
    # Exact candidate metadata reviewed on 2026-10-25. Identity's new event is
    # deliberately appended locally; upstream inserted it among old variants.
    reviewed_additions = []
    for category, name, index, field_name, compact in (
        ("calls", "remove_registrar", 24, "index", True),
        ("event", "RegistrarRemoved", 22, "registrar_index", False),
    ):
        old_variants = old_types[old_pallets["Identity"][category]["ty"]]["type"]["def"]["variant"]["variants"]
        new_definition = types[new_pallets["Identity"][category]["ty"]]["type"]["def"]["variant"]
        new_variants = new_definition["variants"]
        indexed(new_variants, "index")
        if any(value["index"] == index or value["name"] == name for value in old_variants):
            raise ValueError("Identity addition must use an unused index: " + name)
        matches = [value for value in new_variants if value["index"] == index or value["name"] == name]
        if len(matches) != 1 or matches[0]["index"] != index or matches[0]["name"] != name:
            raise ValueError("Identity addition must retain its reviewed name and index: " + name)
        fields = matches[0].get("fields", [])
        if len(fields) != 1 or fields[0].get("name") != field_name:
            raise ValueError("Identity addition must have its exact single field: " + name)
        definition = types[fields[0]["type"]]["type"]["def"]
        if compact:
            if set(definition) != {"compact"}:
                raise ValueError("Identity.remove_registrar index must be Compact<u32>")
            definition = types[definition["compact"]["type"]]["type"]["def"]
        if definition != {"primitive": "u32"}:
            raise ValueError("Identity registrar index must encode u32: " + name)
        new_definition["variants"] = [value for value in new_variants if value["name"] != name]
        reviewed_additions.append({"path": "Identity." + category, "variant": name,
                                   "variantIndex": index, "field": field_name,
                                   "fieldEncoding": "Compact<u32>" if compact else "u32"})
    result = Comparison(old, projected).run()
    result["reviewedSdkMigrationCompatible"] = result["compatibleExistingScaleEncoding"] and not result["additions"]
    result["compatibleExistingScaleEncoding"] = raw["compatibleExistingScaleEncoding"]
    result["rawBreakingChanges"] = raw["breakingChanges"]
    result["rawAdditions"] = raw["additions"]
    result["reviewedAdditions"] = reviewed_additions
    result["reviewedEventChanges"] = [
        {"event": "System.CodeUpdated", "index": 2, "change": "unit to {hash: runtime Hash}",
         "binaryCompatible": False, "requiresMetadataRefresh": True},
        {"event": "Offences.Offence", "index": 0, "change": "timeslot field renamed slot with unchanged type",
         "binaryCompatible": True, "requiresMetadataRefresh": True},
    ]
    result["reviewedSignedExtensionChange"] = {
        "added": "AuthorizeCall", "position": 0,
        "explicitEncodedBytes": 0, "implicitEncodedBytes": 0,
        "existingOrderAndTypesPreserved": result["reviewedSdkMigrationCompatible"],
        "candidateIdentifiers": expected,
        "requiresTransactionVersion": 132,
    }
    return result


# The fork keeps metadata wrappers for every historical prefix. No storage
# retirement is approved: legacy keys and their public query encoding stay intact.
ASYNC_LEGACY_STORAGE = set()

# Reviewed against diagnostic runtime137/133, SHA256
# c040d5621bfce585dac66a90f74490e528ce5ffbff172564c98bedc555823da6.
# These58 exact additions comprise7calls,11events,9errors,24storageitems,
# 6constants and the storage-only bridge. Every nested shape is fixed; existing
# calls/events/errors/storage remain subject to the ordinary strict comparison.
ASYNC_ADDITION_SHAPES = {
    "Staking.calls.apply_slash": "dc2fd8db65fc2b3692a0922dc5850f8049dd6a9fb3cd00d64fda4592bbbaa4d7",
    "Staking.calls.cancel_async_deferred_slash": "e07318bead66c4bfdd9432ed37560fe51fb1e30e813982b6329c3e509845058f",
    "Staking.calls.chill_inactive": "62ca6aece80a31ca53397794014c39223b31e7337a07b97f0d0c519fe4ca8c91",
    "Staking.calls.prune_era_step": "35c3a998bb90b98d375550460e10420c44b6de718583dcdca10bf0992cb8131c",
    "Staking.calls.set_async_staking_configs": "97568081a5f65e92f5b26fd7e66e6aad0afd21b7e7be4a43ade6cc2e1ae9b222",
    "Staking.calls.set_max_commission": "760f339facb2cef4f81f6de88ba7caa6af0282976a04bbaa82d15629474bf223",
    "Staking.calls.set_validator_self_stake_incentive_config": "788308370ca3efddb2b7160eec406969b18d4d411b00aafa1d7af44a04de5de6",
    "Staking.constants.DisableMinting": "0af079e9d0e2e331b608714c88a6050ebdd6705f53137ee54e4a49dfed5e51e9",
    "Staking.constants.MaxEraDuration": "865f542f7cfeada9280ed113b2c70c343ed0c3363415f8391480e6b7f560f34b",
    "Staking.constants.MaxPruningItems": "89c1ca84c12a3fa924bc17266711f5985767474c74ab12bde58e673958e66510",
    "Staking.constants.NominatorFastUnbondDuration": "c9ae2952c76cc596fca4da9c3095f458c9d5d046d9312d1212161add4eac0187",
    "Staking.constants.PlanningEraOffset": "4687287ac80e4f62c4602dba2d169778892f58ddc2d76ce7794027f4e1f73d62",
    "Staking.constants.SlashDeferOffset": "dbf098852d0c00702537e99f5c77a09ed1fcd988f2ceab54c77be3ef0310f0b3",
    "Staking.error.CancelledSlash": "6564709b29c44f1e5876161fb8027240097b6a97b4f45a1cbda68f75c2f36f5e",
    "Staking.error.CommissionTooHigh": "4b5a783370943fcfbb8a1670259f5bbee9765f99e629428d2cc183bcd51fa77c",
    "Staking.error.EraNotPrunable": "40ca87204e085827587df3b0cce9854d0ae9992f4535b1f32cd6e712fe58af66",
    "Staking.error.EraNotStarted": "6b5350c7a864fa05cec693b1980518b98e48a947d75f8ffe41df28af7a614430",
    "Staking.error.InvalidChillInactiveThreshold": "ded0d4b5606107ef8fe11e257287fed64cc59155de52270784ddd586c4c13213",
    "Staking.error.InvalidInactivityProof": "a322aa6bb73df451231cd8d06e909954002a448010e8c453c2c58969b6e2f95d",
    "Staking.error.InvalidSlashRecord": "cad2965edbac5d5deed8297f471d3d190d2e3442624dedaa9229d624ff3d0341",
    "Staking.error.OptimumGreaterThanCap": "84a9c016dd91a377cd0abf4641e8c88ead279b35d5cd4f226094c424b5674fc8",
    "Staking.error.UnappliedSlashesInPreviousEra": "41071920c6f08e28ef629e87e36f43b64ce6b203d9b680e2b9725760311dea0d",
    "Staking.event.EraPruned": "50ac6103f38a5a6baa61ad8b823a285eb14e10d5cd5b68f6695285b944cf1fea",
    "Staking.event.OffenceReported": "4b5960fa3bd4a822901431c89876730f4c9230c3453d7af7326f9c74cc1eaed7",
    "Staking.event.OffenceTooOld": "2ac69bba1bfb42e7559a5eacd5391f66bfe0b95ba022314b0850249ba903acb5",
    "Staking.event.PagedElectionProceeded": "52344f15968baeac59fdc5fc11809700a2b4d46699e2c567c013d79579020dfd",
    "Staking.event.SessionRotated": "bef453821788e9a6c1a47f70defaeefe484e984f98c651c94a966970cabf89d3",
    "Staking.event.SlashCancelled": "c9badc284327be20a43e5abe71ff25954731a1b793a50c3abb9fbf6b4332333e",
    "Staking.event.SlashComputed": "419af5753f6b956b6ccf717e39ca3de5e6e62c7f4ac3f1d6d655fdf96912077b",
    "Staking.event.StakerRemoved": "b49c419e2225db4635eba2b23fe022fd137b8dc17c0779367a7a925f1d4d1e53",
    "Staking.event.Unexpected": "d914b9fc2eef9cbdd8b7d91afbf43c16b760054ba2c598c2e86895cb66f622e5",
    "Staking.event.ValidatorIncentiveConfigSet": "d962f5097267ac5afacea5dd32e4afa08b78107286352cf55872aab5df0f94b3",
    "Staking.event.ValidatorIncentivePaid": "566840d13785a13e530b05b9128cbc34aa85beb9a898c3ef1a505e36fbd1f7ef",
    "Staking.storage.AreNominatorsSlashable": "aed8d60c55fc3fee30e74109ca6a490d09724921a63420022aaa86c918f245b5",
    "Staking.storage.AsyncUnappliedSlashes": "3a458400e8d738946e89b59649d187e87f30242e25878bffe774feb6896ac3de",
    "Staking.storage.CancelledSlashes": "0d706558e00e31c3e25617fb8d2b94a56d1cc611b045b366bb611156b3cd3b60",
    "Staking.storage.ChillInactiveThreshold": "d194d8f9e32d96803897b5897b499de33e343dc83263b2df69fb23c94cff5785",
    "Staking.storage.DisableMintingGuard": "eea0f94b625aee236824d9301ee1e87abcd6b9720f042d4349d8be1e9b9e07ce",
    "Staking.storage.ElectableStashes": "7a06c80547efd6aff9fbbb66d65fa9a82df5688d7cc9dd59ab3bd8bd93113657",
    "Staking.storage.EraPruningState": "4dcd8fc3df91d4e52cbcb87061109ac894a52641738d5a8f49df735a8839abfd",
    "Staking.storage.ErasNominatorsSlashable": "2b255eff8d8303c0b040d2673cb67f14065a9bc327f91e49e730de3e5e80b30e",
    "Staking.storage.ErasSumValidatorIncentiveWeight": "ecdba6bbf056d89189cd5395c996bdf14208186c1aa0ef149d7eb3428d80a4b6",
    "Staking.storage.ErasSumWeightedPoints": "b08186486ee8dde5c637f76ce95949756ad885c83b2918800a481cb7b3782eb1",
    "Staking.storage.ErasValidatorIncentiveBudget": "6fae4c0d721f27f26f673da3a379a2dae52470eb8ebf76438d092ff3b27559a0",
    "Staking.storage.ErasValidatorIncentiveWeight": "dc43ca846bd1989af24f89f01b24b3ea58c1a2ff12d1e3e350ea806751b82a16",
    "Staking.storage.HardCapSelfStake": "2e73cd9aa522fba0db3a40cf5c1a28a04be4f3671037adcee1a0c84be8eb715b",
    "Staking.storage.LastValidatorEra": "6d071ac421fb4723a2951c981e62cf439a42eafbeedb3a1077c0e129abe9db38",
    "Staking.storage.MaxCommission": "f2fdbac86d31c837d31829c653bcdf1bf5fe7754849d23b6b25ea45e57366cd7",
    "Staking.storage.NextDeferredSlashEra": "b7503225d52776527d529e511f355c0bd08031ff0e6ffaeaea2dc20431a50293",
    "Staking.storage.NextElectionPage": "33cddd64cf8071cd4518ff4ab3b12e41c3d0ef45ffa9c3862955d30cc1ffdc29",
    "Staking.storage.OffenceQueue": "4646696cc94c4275ebe68a219b60b32f195aa653beeca748fafdbfa7d26e6d4f",
    "Staking.storage.OffenceQueueEras": "258274195c77c6da511c49ee42c821ccf53810007bb62da15f6549d0e43a5432",
    "Staking.storage.OptimumSelfStake": "e72daa26e164fdf18b1769fbdf67ebb50464a4b7e0e5c8b7b25cb2999daab8b4",
    "Staking.storage.ProcessingOffence": "09c183a2216b4f67236dc23b1fe04cf379c190b45793c3eabbdd8d311d487d30",
    "Staking.storage.SelfStakeSlopeFactor": "9e6cabc888f92122d13e5aa38fcdf0ff5ff540b12d9ea0199cd6da0f7ef92c92",
    "Staking.storage.VoterSnapshotStatus": "23ea174bb5be22c6c7e623d8d957869665cfaa51e59310b7d4dd53b1f9cf59b4",
    "Staking.storage.WeightedPointsFormulaStartEra": "f89faf4550ff768b43ff9367ad46467635c795760e79517668ead971b869718f",
    "StakingAsyncBridge": "6e25f9044f4c760cef8fd22078cdd5a8da8e9b256ff34221670381b05d258c21"
}


def type_shape(metadata, type_id, stack=()):
    types = {entry["id"]: entry["type"] for entry in metadata["types"]["types"]}
    if type_id in stack:
        return {"recursiveAncestor": len(stack) - 1 - stack.index(type_id)}
    definition = copy.deepcopy(types[type_id]["def"])
    kind, value = next(iter(definition.items()))
    recurse = lambda reference: type_shape(metadata, reference, (*stack, type_id))
    if kind in ("array", "sequence", "compact"):
        value["type"] = recurse(value["type"])
    elif kind == "tuple":
        definition[kind] = [recurse(reference) for reference in value]
    elif kind == "composite":
        definition[kind] = {"fields": [{"name": field.get("name"), "type": recurse(field["type"])} for field in value.get("fields", [])]}
    elif kind == "variant":
        definition[kind] = {"variants": [{"index": variant["index"], "name": variant["name"],
            "fields": [{"name": field.get("name"), "type": recurse(field["type"])} for field in variant.get("fields", [])]}
            for variant in value.get("variants", [])]}
    elif kind in ("bitsequence", "bitSequence"):
        store, order = bit_sequence_refs(value)
        definition = {"bitSequence": {"store": recurse(store), "order": recurse(order), "orderPath": types[order].get("path")}}
    elif kind != "primitive":
        raise ValueError("Unknown type kind in async review: " + kind)
    return definition


def addition_shape(metadata, category, value):
    if category in ("calls", "event", "error"):
        return {"index": value["index"], "name": value["name"], "fields": [
            {"name": field.get("name"), "type": type_shape(metadata, field["type"])} for field in value.get("fields", [])]}
    if category == "storage":
        kind, stored = next(iter(value["ty"].items()))
        definition = ({"Plain": type_shape(metadata, stored)} if kind == "Plain" else
            {"Map": {"hashers": stored["hashers"], "key": type_shape(metadata, stored["key"]), "value": type_shape(metadata, stored["value"])}})
        return {"name": value["name"], "modifier": value["modifier"], "default": value["default"], "ty": definition}
    if category == "constants":
        return {"name": value["name"], "value": value["value"], "type": type_shape(metadata, value["ty"])}
    if category == "pallet":
        return {"name": value["name"], "index": value["index"],
            "storage": {"prefix": value["storage"]["prefix"], "entries": [addition_shape(metadata, "storage", entry) for entry in value["storage"]["entries"]]},
            "constants": [addition_shape(metadata, "constants", entry) for entry in value.get("constants", [])],
            "calls": value.get("calls"), "event": value.get("event"), "error": value.get("error")}
    raise ValueError("Unknown async review category " + category)


def shape_digest(shape):
    return hashlib.sha256(json.dumps(shape, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def async_staking_comparison(old, new):
    baseline, projected = copy.deepcopy(old), copy.deepcopy(new)
    old_pallets, new_pallets = indexed(baseline["pallets"], "name"), indexed(projected["pallets"], "name")
    old_types = indexed(baseline["types"]["types"], "id")
    new_types = indexed(projected["types"]["types"], "id")
    before, after = old_pallets["Staking"], new_pallets["Staking"]
    if before["index"] != 17 or after["index"] != 17:
        raise ValueError("Staking must retain pallet index17")
    reviewed = []
    found = set()
    def approve(category, entry, path):
        actual = shape_digest(addition_shape(new, category, entry))
        if ASYNC_ADDITION_SHAPES.get(path) != actual:
            raise ValueError("Unreviewed async addition or encoding drift: " + path + " (" + actual + ")")
        found.add(path)
        reviewed.append({"path": path, "shapeSha256": actual})
    old_entries = indexed(before["storage"]["entries"], "name")
    new_entries = indexed(after["storage"]["entries"], "name")
    removed = set(old_entries) - set(new_entries)
    if removed != ASYNC_LEGACY_STORAGE:
        raise ValueError("Unexpected legacy metadata retirement: " + str(sorted(removed)))
    before["storage"]["entries"] = [entry for entry in before["storage"]["entries"] if entry["name"] not in ASYNC_LEGACY_STORAGE]
    for category in ("calls", "event", "error"):
        previous = old_types[before[category]["ty"]]["type"]["def"]["variant"]["variants"]
        definition = new_types[after[category]["ty"]]["type"]["def"]["variant"]
        previous_indices = {entry["index"] for entry in previous}
        added = [entry for entry in definition["variants"] if entry["index"] not in previous_indices]
        for entry in added:
            approve(category, entry, "Staking." + category + "." + entry["name"])
        definition["variants"] = [entry for entry in definition["variants"] if entry["index"] in previous_indices]
    for category in ("storage", "constants"):
        old_values = before["storage"]["entries"] if category == "storage" else before[category]
        new_values = after["storage"]["entries"] if category == "storage" else after[category]
        names = {entry["name"] for entry in old_values}
        added = [entry for entry in new_values if entry["name"] not in names]
        for entry in added:
            approve(category, entry, "Staking." + category + "." + entry["name"])
        kept = [entry for entry in new_values if entry["name"] in names]
        if category == "storage": after["storage"]["entries"] = kept
        else: after[category] = kept
    bridge = new_pallets.get("StakingAsyncBridge")
    if not bridge or bridge["index"] != 121 or any(bridge.get(key) for key in ("calls", "event", "error")):
        raise ValueError("Expected storage-only local bridge at index121")
    approve("pallet", bridge, "StakingAsyncBridge")
    projected["pallets"] = [pallet for pallet in projected["pallets"] if pallet["name"] != "StakingAsyncBridge"]
    if found != set(ASYNC_ADDITION_SHAPES):
        raise ValueError("A reviewed async addition is missing")
    # Only these three existing storage values gained upstream bounded-container
    # metadata wrappers. Their codecs add no bytes. Unwrap exactly one unnamed
    # field at these roots, then run the ordinary strict comparison on every
    # inner field, storage key, hasher, modifier and default.
    wrappers = []
    def bounded_inner(type_id, module, name, path):
        wrapped = new_types[type_id]["type"]
        fields = wrapped.get("def", {}).get("composite", {}).get("fields", [])
        if wrapped.get("path") != ["bounded_collections", module, name] or len(fields) != 1 or fields[0].get("name") is not None:
            raise ValueError("Unexpected bounded storage metadata wrapper: " + path)
        wrappers.append({"path": path, "wrapper": "::".join(wrapped["path"]),
            "shapeSha256": shape_digest(type_shape(new, type_id)),
            "change": "One unnamed codec-transparent container field; entire inner encoding remains strictly compared"})
        return fields[0]["type"]
    after_entries = indexed(after["storage"]["entries"], "name")
    bonded = after_entries["BondedEras"]["ty"]
    bonded["Plain"] = bounded_inner(bonded["Plain"], "bounded_vec", "BoundedVec", "Staking.storage.BondedEras")
    claimed = after_entries["ClaimedRewards"]["ty"]["Map"]
    claimed["value"] = bounded_inner(claimed["value"], "weak_bounded_vec", "WeakBoundedVec", "Staking.storage.ClaimedRewards")
    points_type = after_entries["ErasRewardPoints"]["ty"]["Map"]["value"]
    points_fields = new_types[points_type]["type"]["def"]["composite"]["fields"]
    if [field.get("name") for field in points_fields] != ["total", "individual"]:
        raise ValueError("Reward points fields changed")
    points_fields[1]["type"] = bounded_inner(points_fields[1]["type"], "bounded_btree_map", "BoundedBTreeMap", "Staking.storage.ErasRewardPoints.individual")
    result = sdk2609_comparison(baseline, projected)
    raw = Comparison(old, new).run()
    result["reviewedAsyncStakingMigrationCompatible"] = result["reviewedSdkMigrationCompatible"]
    result["rawBreakingChanges"] = raw["breakingChanges"]
    result["rawAdditions"] = raw["additions"]
    result["compatibleExistingScaleEncoding"] = raw["compatibleExistingScaleEncoding"]
    result["reviewedAsyncAdditions"] = reviewed
    result["reviewedStorageMetadataWrappers"] = wrappers
    result["retainedLegacyStorageOutsideMetadata"] = sorted(ASYNC_LEGACY_STORAGE)
    result["requiresStorageMigration"] = {"pallet": "Staking", "from": 16, "to": 18, "rawStateWitnessRequired": True}
    result["reviewedSignedExtensionChange"]["requiresTransactionVersion"] = 133
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


def retirement_self_checks(metadata):
    extended = copy.deepcopy(metadata)
    types = {item["id"]: item["type"] for item in extended["types"]["types"]}
    primitive = lambda kind: next(index for index, value in types.items() if value["def"] == {"primitive": kind})
    pallets = indexed(extended["pallets"], "name")
    for name, index in RETIREMENT_ERRORS.items():
        pallet = pallets[name]
        pallet["constants"].append({"name": "RepaymentOnly", "ty": primitive("bool"), "value": [1]})
        types[pallet["error"]["ty"]]["def"]["variant"]["variants"].append({"name": "RepaymentOnly", "index": index})
    apollo = pallets["ApolloPlatform"]
    asset_key = next(value for value in apollo["storage"]["entries"] if value["name"] == "PoolData")["ty"]["Map"]["key"]
    apollo["storage"]["entries"].append({"name": "DeferredProtocolInterest", "modifier": "Default",
        "ty": {"Map": {"hashers": ["Identity"], "key": asset_key, "value": primitive("u128")}}, "default": [0] * 16})
    assert retirement_comparison(metadata, extended)["compatibleExistingScaleEncoding"]
    checks = ["Exact typed retirement additions accepted with all existing ABI preserved"]

    def rejects(label, mutate):
        changed = copy.deepcopy(extended)
        mutate(changed)
        try:
            accepted = retirement_comparison(metadata, changed)["compatibleExistingScaleEncoding"]
        except (ValueError, KeyError):
            accepted = False
        assert not accepted, label
        checks.append(label)

    def pallet(model, name):
        return next(value for value in model["pallets"] if value["name"] == name)

    def definition(model, type_id):
        return next(value["type"]["def"] for value in model["types"]["types"] if value["id"] == type_id)

    def variants(model, name, category):
        return definition(model, pallet(model, name)[category]["ty"])["variant"]["variants"]

    def retirement_constant(model):
        return next(value for value in pallet(model, "Kensetsu")["constants"] if value["name"] == "RepaymentOnly")

    def deferred(model):
        return next(value for value in pallet(model, "ApolloPlatform")["storage"]["entries"] if value["name"] == "DeferredProtocolInterest")

    rejects("Legacy storage deletion rejected", lambda model: pallet(model, "IrohaMigration")["storage"]["entries"].pop(0))
    rejects("Legacy migrate argument drift rejected", lambda model: variants(model, "IrohaMigration", "calls")[0]["fields"][0].update(name="changed_argument"))
    rejects("Signed-extension reorder rejected", lambda model: model["extrinsic"]["signed_extensions"].reverse())
    def signature_drift(model):
        extrinsic = next(value["type"] for value in model["types"]["types"] if value["id"] == model["extrinsic"]["ty"])
        next(value for value in extrinsic["params"] if value["name"] == "Signature")["type"] = primitive("u128")
    rejects("Extrinsic signature layout drift rejected", signature_drift)
    rejects("Existing event index drift rejected", lambda model: variants(model, "Kensetsu", "event")[0].update(index=250))
    def constant_drift(model):
        version = next(value for value in pallet(model, "System")["constants"] if value["name"] == "Version")
        version["value"][0] ^= 1
    rejects("Existing constant value change rejected", constant_drift)
    rejects("Unrequested call addition rejected", lambda model: variants(model, "IrohaMigration", "calls").append({"name": "unrequested", "index": 42}))
    rejects("Retirement constant false rejected", lambda model: retirement_constant(model).update(value=[0]))
    rejects("Retirement constant wrong type rejected", lambda model: retirement_constant(model).update(ty=primitive("u8")))
    rejects("Retirement error wrong index rejected", lambda model: variants(model, "ApolloPlatform", "error")[-1].update(index=37))
    rejects("Retirement error payload rejected", lambda model: variants(model, "Kensetsu", "error")[-1].update(fields=[{"type": primitive("u128")}]))
    rejects("Deferred interest wrong key rejected", lambda model: deferred(model)["ty"]["Map"].update(key=primitive("u128")))
    rejects("Deferred interest wrong balance type rejected", lambda model: deferred(model)["ty"]["Map"].update(value=primitive("u64")))
    rejects("Deferred interest wrong hasher rejected", lambda model: deferred(model)["ty"]["Map"].update(hashers=["Blake2_128Concat"]))
    rejects("Deferred interest wrong default rejected", lambda model: deferred(model).update(default=[1] + [0] * 15))
    rejects("Deferred interest optional storage rejected", lambda model: deferred(model).update(modifier="Optional", default=[0]))
    return checks


def main():
    here = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old", type=Path, default=here / "reference-runtime-130/framenode-runtime-4.8.8-metadata.json")
    parser.add_argument("--new", type=Path, default=here.parent / "framenode-runtime-4.8.9-metadata.json")
    parser.add_argument("--output", type=Path, default=here / "metadata-compatibility.json")
    parser.add_argument("--allow-retirement-additions", action="store_true")
    parser.add_argument("--sdk2609", action="store_true")
    parser.add_argument("--async-staking", action="store_true")
    args = parser.parse_args()
    old, new = load(args.old), load(args.new)
    if sum((args.allow_retirement_additions, args.sdk2609, args.async_staking)) > 1:
        parser.error("Compatibility profiles cannot be combined")
    result = (async_staking_comparison(old, new) if args.async_staking else
              sdk2609_comparison(old, new) if args.sdk2609 else
              retirement_comparison(old, new) if args.allow_retirement_additions else Comparison(old, new).run())
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
    if args.allow_retirement_additions:
        result["selfChecks"].extend(retirement_self_checks(old))
    result["comparisonMode"] = ("async-staking-v16-v18-reviewed-changes" if args.async_staking else
                                "sdk2609-reviewed-interface-changes" if args.sdk2609 else
                                "exact-lending-retirement-additions" if args.allow_retirement_additions else "strict")
    result["limitations"] = [
        "Structural metadata comparison verifies declared SCALE layouts, indices, field order/names, storage hashers/defaults and signed payload shapes. It does not execute migrations or verify behavior or custom codec implementations.",
        "New enum variants preserve encoding of old values. Older metadata cannot decode those new variants.",
        "Source paths, documentation, generic type IDs and typeName aliases are ignored; extrinsic Address/Call/Signature/Extra generic parameters are explicitly traversed.",
        "Constant values may intentionally change. Their encoded types are checked and changed bytes are reported separately.",
    ]
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"compatibleExistingScaleEncoding": result["compatibleExistingScaleEncoding"],
                      "reviewedSdkMigrationCompatible": result.get("reviewedSdkMigrationCompatible"),
                      "counts": result["counts"], "breakingChanges": result["breakingChanges"],
                      "additions": len(result["additions"]), "constantValueChanges": len(result["constantValueChanges"]),
                      "selfChecks": len(result["selfChecks"]), "output": str(args.output)}, indent=2))
    return 0 if result.get("reviewedSdkMigrationCompatible", result["compatibleExistingScaleEncoding"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
