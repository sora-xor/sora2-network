#!/usr/bin/env python3
"""Independently match the exact-Wasm fee events to balances and duplicate errors."""
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def number(value):
    return int(value, 0) if isinstance(value, str) and value.startswith("0x") else int(value)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def validate_bindings(root):
    if not __debug__:
        raise RuntimeError("Fee evidence verification requires assertions; do not use -O/PYTHONOPTIMIZE")
    raw = (root / "equivocation-fee-wasm-rehearsal.json").read_bytes()
    rehearsal = json.loads(raw)
    compatibility_raw = (root / "wasm-compatibility.json").read_bytes()
    compatibility = json.loads(compatibility_raw)
    snapshot_raw = (root / "snapshot.json").read_bytes()
    snapshot = json.loads(snapshot_raw)
    assert rehearsal["status"] == compatibility["status"] == snapshot["status"] == "passed"
    candidate_hash = sha256((root.parent / "candidate.compact.compressed.wasm").read_bytes())
    assert candidate_hash == rehearsal["candidate"]["sha256"] == compatibility["inputs"]["candidate"]["sha256"], "Candidate identity mismatch"
    snapshot_hash = sha256(snapshot_raw)
    assert snapshot_hash == rehearsal["inputs"]["snapshotSha256"] == compatibility["snapshotSha256"], "Snapshot identity mismatch"
    assert sha256((root / "live-runtime.compact.compressed.wasm").read_bytes()) == snapshot["code"]["sha256"] == compatibility["inputs"]["baseline"]["sha256"], "Deployed code identity mismatch"
    metadata_raw = (root / "candidate-metadata.json").read_bytes()
    assert sha256(metadata_raw) == compatibility["inputs"]["candidate"]["metadataSha256"], "Candidate metadata identity mismatch"
    return rehearsal, json.loads(metadata_raw)["V14"], {
        "candidateSha256": candidate_hash, "snapshotSha256": snapshot_hash,
        "metadataSha256": sha256(metadata_raw), "compatibilitySha256": sha256(compatibility_raw),
        "rehearsalSha256": sha256(raw),
    }


def verify(root):
    rehearsal, metadata, bindings = validate_bindings(root)
    types = {entry["id"]: entry["type"] for entry in metadata["types"]["types"]}
    errors = {}
    for pallet in metadata["pallets"]:
        if pallet["name"] in {"Babe", "Grandpa"}:
            errors[pallet["name"].lower()] = {
                "pallet": pallet["index"],
                "duplicate": next(v["index"] for v in
                    types[pallet["error"]["ty"]]["def"]["variant"]["variants"]
                    if v["name"] == "DuplicateOffenceReport"),
            }
    rows = []
    for row in rehearsal["signedChecks"]:
        assert row["passed"] and row["signed"]
        events = [event for event in row["events"] if
                  event["section"] == "transactionPayment" and event["method"] == "TransactionFeePaid"]
        assert len(events) == 1, row["method"]
        payer, fee, tip = events[0]["data"]
        charged = number(row["xorCharged"])
        before = row["signerBefore"]
        after = row["signerAfter"]
        assert payer == rehearsal["signer"]["address"], row["method"]
        assert charged > 0 and number(fee) == charged and number(tip) == 0, row["method"]
        assert number(before["data"]["free"]) - number(after["data"]["free"]) == charged
        assert number(after["nonce"]) == number(before["nonce"]) + 1
        dispatch = row["outcome"]["ok"]
        if row["expectSuccess"]:
            assert "ok" in dispatch, row["method"]
        else:
            assert row["method"].endswith(" duplicate")
            error = dispatch["err"]["module"]
            expected = errors[row["method"].split(".", 1)[0]]
            assert error["index"] == expected["pallet"]
            assert bytes.fromhex(error["error"][2:]) == bytes([expected["duplicate"], 0, 0, 0])
        rows.append({"method": row["method"], "payer": payer,
                     "actualFee": str(charged), "eventEqualsBalanceDecrease": True,
                     "dispatchOutcomeChecked": True})
    assert len(rows) == 9
    report = {"status": "passed", "checkedSignedReports": len(rows),
              **bindings,
              "scriptSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "allFeeEventsMatchPayerBalanceDecrease": True,
              "allFailedReportsAreDuplicateOffenceReport": True,
              "networkRequests": 0, "checks": rows}
    return report


def main(root=ROOT):
    report = {"status": "running", "startedAt": datetime.now(timezone.utc).isoformat()}
    output = root / "fee-event-check.json"
    output.write_text(json.dumps(report, indent=2) + "\n")
    code = 0
    try:
        report.update(verify(root))
    except Exception as error:
        report.update(status="failed", error={"type": type(error).__name__, "message": str(error)})
        code = 1
    finally:
        report["finishedAt"] = datetime.now(timezone.utc).isoformat()
        report["scriptSha256"] = sha256(Path(__file__).read_bytes())
        output.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps({"status": report["status"], "feeEventsChecked": report.get("checkedSignedReports"), "candidateSha256": report.get("candidateSha256"), "error": report.get("error")}))
    return code


if __name__ == "__main__":
    sys.exit(main())
