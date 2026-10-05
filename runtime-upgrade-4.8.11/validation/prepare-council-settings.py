#!/usr/bin/env python3
"""Render council settings from verified-candidate and finalized-governance inputs."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
info = json.loads((ROOT / "runtime-upgrade-4.8.11-info.json").read_text())
gov = json.loads((ROOT / "governance-calls.json").read_text())
assert gov["status"] == "passed"
assert gov["setCodeProposalHash"] == info["proposal"]["hash"]
assert gov["nextExternal"] is None and gov["candidateBlacklist"] is None
assert gov["submittedTransactions"] == 0

settings = {
    "release": "4.8.11", "signed": False, "submittedTransactions": 0,
    "network": {"name": "SORA mainnet", "genesisHash": gov["genesis"],
                "endpoint": gov["endpoint"], "finalizedBlock": gov["blockNumber"],
                "finalizedBlockHash": gov["blockHash"], "checkedAt": gov["checkedAt"]},
    "runtime": {"baselineSpecVersion": 132, **info["candidate"]},
    "preimage": {"method": "preimage.notePreimage", "bytesArgumentFile": "set-code-call.hex",
                 "completeCallFile": "preimage-note-call.hex", "hash": info["proposal"]["hash"],
                 "hashAlgorithm": "BLAKE2-256 of full SCALE system.setCode call",
                 **gov["preimage"]},
    "council": {"method": "council.propose", "callFile": gov["preferredCouncilCall"],
                "threshold": gov["minimumCouncilThreshold"], "members": len(gov["council"]["members"]),
                "innerCallFile": "utility-guarded-external-majority-call.hex",
                "innerCall": "utility.batchAll([democracy.externalPropose(candidate), democracy.externalProposeMajority(candidate)])",
                "proposalLengthBound": gov["calls"]["utility-guarded-external-majority-call"]["bytes"],
                "motionHashForVoteAndClose": gov["preferredCouncilMotionHash"],
                "motionIndex": None, "close": gov["closeBounds"]["council"]},
    "technicalCommittee": {"method": "technicalCommittee.propose",
                "callFile": f'technical-committee-fast-track-threshold-{gov["technicalCommitteeThreshold"]}-call.hex',
                "threshold": gov["technicalCommitteeThreshold"], "members": len(gov["technicalCommittee"]["members"]),
                "innerCallFile": "democracy-fast-track-call.hex", "innerCall": "democracy.fastTrack",
                "proposalHash": info["proposal"]["hash"], "votingPeriodBlocks": gov["fastTrackVotingPeriod"],
                "enactmentDelayBlocks": gov["enactmentDelay"],
                "proposalLengthBound": gov["calls"]["democracy-fast-track-call"]["bytes"],
                "motionHashForVoteAndClose": gov["technicalMotionHash"],
                "motionIndex": None, "close": gov["closeBounds"]["technicalCommittee"]},
    "referendumIndex": None,
    "instructions": gov["instructions"] + [
        "Null motion/referendum indices are intentionally unset. Read actual Proposed/Started events; never submit null or guessed indices.",
        "Requery live stored-motion weights and membership before signing vote/close calls.",
        "Preimage registration, council approval, technical fast-track, referendum passage and enactment are separate steps.",
    ],
    "inputSha256": {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
                    for name in ["runtime-upgrade-4.8.11-info.json", "governance-calls.json"]},
}
(ROOT / "council-settings.json").write_text(json.dumps(settings, indent=2) + "\n")
print(json.dumps({"status": "prepared", "file": "council-settings.json", "block": gov["blockNumber"]}))
