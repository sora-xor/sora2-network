#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import recovery_feeder as feeder

POLICY = "0x" + "ab" * 32


def status(phase: str) -> str:
    return "0x" + ((1).to_bytes(4, "little") + bytes.fromhex(POLICY[2:]) + bytes([feeder.PHASES.index(phase)])
                   + (0).to_bytes(4, "little") + (0).to_bytes(8, "little") + b"\x00").hex()


def compact(value: int) -> bytes:
    if value < 1 << 6:
        return bytes([value << 2])
    if value < 1 << 14:
        return ((value << 2) | 1).to_bytes(2, "little")
    return ((value << 2) | 2).to_bytes(4, "little")


def remark(payload: bytes) -> str:
    # Bare extrinsic: version, System.remark call index, then the SCALE Vec<u8> payload last.
    body = b"\x05\x00\x01" + compact(len(payload)) + payload
    return "0x" + (compact(len(body)) + body).hex()


class Chain:
    def __init__(self):
        self.blocks, self.status, self.head = {}, {feeder.CHECKPOINT_HASH: None}, feeder.CHECKPOINT_HASH

    def add(self, parent: str, payloads: list[bytes], phase: str = "Applying") -> str:
        number = feeder.CHECKPOINT_NUMBER + 1 + sum(1 for _ in self.ancestors(parent)) - 1
        hash_ = "0x" + hashlib.sha256(f"{parent}{number}{payloads}".encode()).hexdigest()
        self.blocks[hash_] = {"header": {"parentHash": parent, "number": hex(number)},
                              "extrinsics": ["0x0c0000"] + [remark(p) for p in payloads]}
        self.status[hash_], self.head = status(phase), hash_
        return hash_

    def ancestors(self, hash_: str):
        while hash_ != feeder.CHECKPOINT_HASH:
            yield hash_
            hash_ = self.blocks[hash_]["header"]["parentHash"]
        yield hash_

    def rpc(self, method, params):
        if method == "chain_getBlock":
            return {"block": self.blocks[params[0]]}
        if method == "state_getStorage":
            return self.status[params[1]]
        return self.head


class FeederTest(unittest.TestCase):
    def setUp(self):
        self.work = [feeder.IDENTIFIER + bytes([1, 2, i]) * 40 for i in range(3)]
        self.temp = tempfile.TemporaryDirectory()
        self.dir = Path(self.temp.name)
        self.chain = Chain()
        self.feed = feeder.Feeder(self.chain.rpc, {"policyCommitment": POLICY}, self.work, self.dir)

    def tearDown(self):
        self.temp.cleanup()

    def written(self, head: str) -> bytes | None:
        path = self.dir / f"{head}.scale"
        return path.read_bytes() if path.exists() else None

    def test_first_child_ticks_then_work_follows_inclusion_not_time(self):
        self.assertIn("Tick", self.feed.deliver(feeder.CHECKPOINT_HASH))
        self.assertIsNone(self.written(feeder.CHECKPOINT_HASH))
        first = self.chain.add(feeder.CHECKPOINT_HASH, [feeder.TICK])
        self.feed.deliver(first)
        self.assertEqual(self.written(first), self.work[0])
        tick = self.chain.add(first, [feeder.TICK])  # another author ticked: no progress
        self.feed.deliver(tick)
        self.assertEqual(self.written(tick), self.work[0])
        applied = self.chain.add(tick, [self.work[0]])
        self.feed.deliver(applied)
        self.assertEqual(self.written(applied), self.work[1])

    def test_forks_each_receive_their_own_next_envelope(self):
        first = self.chain.add(feeder.CHECKPOINT_HASH, [feeder.TICK])
        applied = self.chain.add(first, [self.work[0]])
        ticked = self.chain.add(first, [feeder.TICK])
        self.feed.deliver(applied); self.feed.deliver(ticked)
        self.assertEqual(self.written(applied), self.work[1])
        self.assertEqual(self.written(ticked), self.work[0])

    def test_completion_and_stops(self):
        head = self.chain.add(feeder.CHECKPOINT_HASH, [feeder.TICK])
        for envelope in self.work:
            head = self.chain.add(head, [envelope])
        self.assertEqual(self.feed.deliver(head), "complete")
        failed = self.chain.add(self.chain.blocks[head]["header"]["parentHash"], [feeder.TICK], "Failed")
        with self.assertRaises(feeder.Halt):
            self.feed.deliver(failed)

    def test_out_of_sequence_work_and_conflicting_files_halt(self):
        first = self.chain.add(feeder.CHECKPOINT_HASH, [feeder.TICK])
        skipped = self.chain.add(first, [self.work[1]])
        with self.assertRaises(feeder.Halt):
            self.feed.deliver(skipped)
        (self.dir / f"{first}.scale").write_bytes(self.work[2])
        with self.assertRaises(feeder.Halt):
            self.feed.deliver(first)

    def test_wrong_policy_and_bundle_tampering_are_refused(self):
        first = self.chain.add(feeder.CHECKPOINT_HASH, [feeder.TICK])
        other = feeder.Feeder(self.chain.rpc, {"policyCommitment": "0x" + "cd" * 32}, self.work, self.dir)
        with self.assertRaises(feeder.Halt):
            other.deliver(first)
        bundle = self.dir / "bundle"; bundle.mkdir()
        rows = []
        for i, raw in enumerate(self.work):
            (bundle / f"{i:05}.scale").write_bytes(raw)
            rows.append({"file": f"{i:05}.scale", "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
        (bundle / "BUNDLE.json").write_text(json.dumps({"schema": "sora-nexus-recovery-work-bundle-v1",
                                                        "checkpointHash": feeder.CHECKPOINT_HASH,
                                                        "policyCommitment": POLICY, "envelopes": rows}))
        self.assertEqual(feeder.load_bundle(bundle)[1], self.work)
        (bundle / "00001.scale").write_bytes(self.work[2])
        with self.assertRaises(feeder.Halt):
            feeder.load_bundle(bundle)


if __name__ == "__main__":
    unittest.main()
