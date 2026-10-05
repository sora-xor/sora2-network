#!/usr/bin/env python3
"""Independently match the exact-Wasm fee events to balances and duplicate errors."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def number(value):
    return int(value, 0) if isinstance(value, str) and value.startswith("0x") else int(value)


def main():
    raw = (ROOT / "equivocation-fee-wasm-rehearsal.json").read_bytes()
    rehearsal = json.loads(raw)
    assert rehearsal["status"] == "passed"
    metadata = json.loads((ROOT / "candidate-metadata.json").read_text())["V14"]
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
              "candidateSha256": rehearsal["candidate"]["sha256"],
              "rehearsalSha256": hashlib.sha256(raw).hexdigest(),
              "scriptSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "allFeeEventsMatchPayerBalanceDecrease": True,
              "allFailedReportsAreDuplicateOffenceReport": True,
              "networkRequests": 0, "checks": rows}
    (ROOT / "fee-event-check.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": "passed", "feeEventsChecked": len(rows),
                      "candidateSha256": report["candidateSha256"]}))


if __name__ == "__main__":
    main()
