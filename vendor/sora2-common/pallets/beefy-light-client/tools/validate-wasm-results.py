#!/usr/bin/env python3
"""Check every recorded Wasm timing against the composed charges in weights.rs."""
import argparse
import hashlib
import json
import pathlib
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    parser.add_argument("--minimum-margin", type=int, default=2)
    args = parser.parse_args()
    pallet = pathlib.Path(__file__).resolve().parents[1]
    root = pathlib.Path(__file__).resolve().parents[5]
    source = (pallet / "src/weights.rs").read_text()
    runtime = (root / "runtime/src/lib.rs").read_text()
    config = re.search(r"impl beefy_light_client::Config for Runtime \{(.*?)\n\}", runtime, re.S)[1]
    maximum = int(re.search(r"type MaxValidators = ConstU32<(\d+)>;", config)[1])
    functions = {}
    for name in ("initialize", "submit_signature_commitment", "verify"):
        body = re.search(r"fn " + name + r"\([^)]*\) -> Weight \{(.*?)\n    \}", source, re.S)[1]
        base, proof = re.search(r"Weight::from_parts\(([\d_]+), ([\d_]+)\)", body).groups()
        terms = {dimension: int(coefficient.replace("_", "")) for coefficient, dimension in re.findall(
            r"Weight::from_parts\(([\d_]+), 0\)\s*\.saturating_mul\((\w+)\.into\(\)\)", body)}
        io = {operation: sum(int(n) for n in re.findall(r"\." + operation + r"\((\d+)\)", body))
              for operation in ("reads", "writes")}
        functions[name] = dict(base=int(base.replace("_", "")), proof=int(proof.replace("_", "")), terms=terms, **io)
    submit = functions["submit_signature_commitment"]
    verify = functions["verify"]
    term = submit["terms"]
    assert set(term) == {"sampling_attempts", "signatures", "proof_nodes", "commitment_bytes"}
    assert set(verify["terms"]) == {"proof_nodes", "digest_logs"}
    signatures = lambda v: (v * 22 + 58) // 59
    expected = {
        "initialize": None, "submit_signature_commitment": ("v", maximum),
        "recover_signatures": ("s", signatures(maximum)),
        "sampling_attempts": ("a", 16 * maximum + 128),
        "merkle_nodes": ("p", 32 * signatures(maximum) + 63),
        "commitment_bytes": ("b", 65536), "commitment_items": ("i", (65536 - 16) // 3),
        "verifier_verify": ("d", 1024),
    }
    batches = json.loads(args.results.read_text())
    assert {batch["benchmark"] for batch in batches} == set(expected), "Missing benchmark case"
    report = {"runtimeMaxValidators": maximum, "minimumRequiredMargin": args.minimum_margin,
              "weightsSha256": hashlib.sha256(source.encode()).hexdigest(),
              "rawResultsSha256": hashlib.sha256(args.results.read_bytes()).hexdigest(),
              "coefficientsPs": functions, "benchmarks": []}
    for batch in batches:
        name = batch["benchmark"]
        rows = batch["time_results"]
        assert len(rows) >= 20, (name, "insufficient repetitions")
        if expected[name]:
            parameter, high = expected[name]
            values = [dict(row["components"])[parameter] for row in rows]
            assert min(values) == 1 and max(values) == high, (name, "range not exercised")
        margins = []
        for row in rows:
            p = dict(row["components"])
            if name == "initialize":
                charged = functions["initialize"]["base"]
            elif name == "verifier_verify":
                charged = verify["base"] + verify["terms"]["proof_nodes"] * 63 + verify["terms"]["digest_logs"] * p["d"]
            else:
                dimensions = {}
                if name == "submit_signature_commitment":
                    # Exclude validator Merkle nodes to obtain a safe lower bound
                    # without assuming a tree depth for non-power-of-two sets.
                    dimensions = dict(sampling_attempts=16 * p["v"] + 128,
                                      signatures=signatures(p["v"]), proof_nodes=63, commitment_bytes=65536)
                elif name == "commitment_items":
                    # A lower bound on encoded bytes; compact length may add more.
                    dimensions = dict(commitment_bytes=3 * p["i"] + 13)
                else:
                    dimension, parameter = {"recover_signatures": ("signatures", "s"),
                                            "sampling_attempts": ("sampling_attempts", "a"),
                                            "merkle_nodes": ("proof_nodes", "p"),
                                            "commitment_bytes": ("commitment_bytes", "b")}[name]
                    dimensions[dimension] = p[parameter]
                charged = submit["base"] + sum(term[key] * value for key, value in dimensions.items())
            observed_ps = row["extrinsic_time"] * 1000
            assert observed_ps > 0
            assert charged >= args.minimum_margin * observed_ps, (name, p, charged, observed_ps)
            margins.append(charged / observed_ps)
        db = batch["db_results"]
        counts = {operation: max(row[operation] for row in db) for operation in ("reads", "writes", "proof_size")}
        model = functions.get(name, verify if name == "verifier_verify" else None)
        if model:
            assert counts["reads"] <= model["reads"] and counts["writes"] <= model["writes"]
            assert counts["proof_size"] <= model["proof"]
        report["benchmarks"].append(dict(name=name, samples=len(rows), minimumMargin=min(margins),
                                         maximumObservedNs=max(row["extrinsic_time"] for row in rows), **counts))
    worst = submit["base"] + term["sampling_attempts"] * (16 * (maximum + 7) + 128) + term["signatures"] * signatures(maximum) + term["proof_nodes"] * (32 * signatures(maximum) + 63) + term["commitment_bytes"] * 65536
    report["worstAdmittedRefTimePsBeforeDb"] = worst
    report["allSamplesPassed"] = True
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"samples": sum(len(batch["time_results"]) for batch in batches),
                      "minimumMargin": min(case["minimumMargin"] for case in report["benchmarks"]),
                      "worstAdmittedRefTimePsBeforeDb": worst, "passed": True}))


if __name__ == "__main__":
    main()
