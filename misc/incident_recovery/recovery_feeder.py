#!/usr/bin/env python3
"""Deliver a release's maintenance work to a validator's recovery inherent directory.

Block authors read the envelope for their exact parent from
`--recovery-inherent-dir/0x<PARENT_HASH>.scale`; an absent file produces a Tick.
This watches the local node, counts the bundle envelopes included on the chain
since the checkpoint, and writes the next envelope for each new best block. It
never signs, submits or chooses work: the order is the rehearsed bundle, and the
runtime authenticates every envelope against the release policy. Standard
library only, so operators can run it beside the node without the repository.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import time
import urllib.request

CHECKPOINT_HASH = "0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0"
CHECKPOINT_NUMBER = 28_191_817
IDENTIFIER = b"snxrec01"
TICK = IDENTIFIER + b"\x01\x00"
STATUS_KEY = "0x" + b":sora-nexus:recovery:status:v1".hex()
PHASES = ("Applying", "Auditing", "Ready", "Hold", "Complete", "Failed")


class Halt(Exception):
    """The chain left the bundle's sequence; stop delivering work."""


def load_bundle(directory: Path) -> tuple[dict, list[bytes]]:
    meta = json.loads((directory / "BUNDLE.json").read_bytes())
    if meta.get("schema") != "sora-nexus-recovery-work-bundle-v1" or meta.get("checkpointHash") != CHECKPOINT_HASH:
        raise Halt("not a recovery work bundle for this checkpoint")
    envelopes = []
    for row in meta["envelopes"]:
        raw = (directory / row["file"]).read_bytes()
        if hashlib.sha256(raw).hexdigest() != row["sha256"] or not raw.startswith(IDENTIFIER) or raw == TICK:
            raise Halt(f"bundle file differs from BUNDLE.json: {row['file']}")
        envelopes.append(raw)
    return meta, envelopes


def compact(raw: bytes, pos: int) -> tuple[int, int]:
    mode = raw[pos] & 3
    if mode == 0:
        return raw[pos] >> 2, pos + 1
    if mode == 1:
        return int.from_bytes(raw[pos:pos + 2], "little") >> 2, pos + 2
    if mode == 2:
        return int.from_bytes(raw[pos:pos + 4], "little") >> 2, pos + 4
    size = (raw[pos] >> 2) + 4
    return int.from_bytes(raw[pos + 1:pos + 1 + size], "little"), pos + 1 + size


def decode_block(raw: bytes) -> tuple[str, int, list[bytes]]:
    """SCALE Block: header (parent, number, roots, digest) then opaque extrinsics."""
    parent = "0x" + raw[:32].hex()
    number, pos = compact(raw, 32)
    pos += 64
    count, pos = compact(raw, pos)
    for _ in range(count):
        kind = raw[pos]; pos += 1
        if kind in (4, 5, 6):
            pos += 4
        if kind in (0, 4, 5, 6):
            size, pos = compact(raw, pos); pos += size
        elif kind != 8:
            raise Halt(f"unknown digest item {kind}")
    count, pos = compact(raw, pos)
    extrinsics = []
    for _ in range(count):
        size, start = compact(raw, pos)
        extrinsics.append(raw[pos:start + size]); pos = start + size
    if pos != len(raw):
        raise Halt("trailing block bytes")
    return parent, number, extrinsics


def advances(extrinsics: list[bytes], expected: bytes | None, where: str) -> bool:
    """True if the block applied the expected envelope; a Tick or no envelope does not advance."""
    advanced = False
    for extrinsic in extrinsics:
        if expected is not None and extrinsic.endswith(expected):
            advanced = True
        elif IDENTIFIER in extrinsic and not extrinsic.endswith(TICK):
            raise Halt(f"block {where} carries recovery work outside the bundle sequence")
    return advanced


def decode_status(value: str | None, policy: str) -> str | None:
    if value is None:
        return None
    raw = bytes.fromhex(value[2:])
    if int.from_bytes(raw[:4], "little") != 1 or "0x" + raw[4:36].hex() != policy or raw[36] >= len(PHASES):
        raise Halt("recovery status has a different policy or format")
    return PHASES[raw[36]]


class Feeder:
    def __init__(self, rpc, meta: dict, envelopes: list[bytes], directory: Path):
        self.rpc, self.policy, self.envelopes, self.directory = rpc, meta["policyCommitment"], envelopes, directory
        self.position = {CHECKPOINT_HASH: (CHECKPOINT_NUMBER, 0)}
        self.done = set()

    def count(self, head: str) -> tuple[int, int]:
        """Bundle envelopes included on the chain from the checkpoint through `head`."""
        pending, cursor = [], head
        while cursor not in self.position:
            block = self.rpc("chain_getBlock", [cursor])["block"]
            number = int(block["header"]["number"], 16)
            if number <= CHECKPOINT_NUMBER:
                raise Halt(f"{head} does not descend from the recovery checkpoint")
            pending.append((cursor, number, block))
            cursor = block["header"]["parentHash"]
        for hash_, number, block in reversed(pending):
            parent_number, included = self.position[block["header"]["parentHash"]]
            if number != parent_number + 1:
                raise Halt(f"block {hash_} has an unexpected number")
            expected = self.envelopes[included] if included < len(self.envelopes) else None
            extrinsics = [bytes.fromhex(e[2:]) for e in block["extrinsics"]]
            self.position[hash_] = (number, included + advances(extrinsics, expected, hash_))
        return self.position[head]

    def deliver(self, head: str) -> str:
        number, included = self.count(head)
        phase = decode_status(self.rpc("state_getStorage", [STATUS_KEY, head]), self.policy)
        if head == CHECKPOINT_HASH:
            return "checkpoint: first child is a Tick"
        if phase in ("Hold", "Failed"):
            raise Halt(f"recovery is {phase} at block {number}; no further work is delivered")
        if phase == "Complete" or included == len(self.envelopes):
            return "complete"
        if phase is None:
            raise Halt(f"no recovery status at block {number}")
        target = self.directory / f"{head}.scale"
        envelope = self.envelopes[included]
        if target.exists():
            if target.read_bytes() != envelope:
                raise Halt(f"{target} holds different work")
            return f"block {number}: envelope {included} already present"
        temporary = self.directory / f".{head}.{os.getpid()}.tmp"
        with temporary.open("xb") as stream:
            stream.write(envelope)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, target)
        return f"block {number} ({phase}): wrote envelope {included}/{len(self.envelopes)} for parent {head}"


def http_rpc(url: str):
    def call(method: str, params: list):
        request = urllib.request.Request(url, json.dumps({"id": 1, "jsonrpc": "2.0", "method": method,
                                                          "params": params}).encode(),
                                         {"Content-Type": "application/json"})
        with urllib.request.urlopen(request, timeout=30) as response:
            reply = json.loads(response.read())
        if "error" in reply:
            raise RuntimeError(f"{method}: {reply['error']}")
        return reply["result"]
    return call


def run(args) -> int:
    meta, envelopes = load_bundle(args.bundle)
    if not args.inherent_dir.is_dir():
        raise Halt("--inherent-dir must be the node's existing --recovery-inherent-dir")
    rpc = http_rpc(args.rpc)
    feeder, last = Feeder(rpc, meta, envelopes, args.inherent_dir), None
    print(f"bundle: {len(envelopes)} envelopes, policy {meta['policyCommitment']}", flush=True)
    while True:
        try:
            head = rpc("chain_getHead", [])
            if head != last:
                message = feeder.deliver(head)
                print(message, flush=True)
                if message == "complete":
                    return 0
                last = head
        except Halt as error:
            print(f"STOPPED: {error}", file=sys.stderr, flush=True)
            return 2
        except (OSError, RuntimeError) as error:
            print(f"retrying after RPC error: {error}", file=sys.stderr, flush=True)
        time.sleep(args.poll)


def verify_replay(bundle: Path, replay: Path) -> dict:
    """Check the bundle order against blocks a rehearsal actually produced and executed."""
    _, envelopes = load_bundle(bundle)
    blocks = sorted(replay.glob("*.block.scale"), key=lambda p: int(p.name.split(".")[0]))
    included, ticks, ordinary, previous = 0, 0, 0, None
    for path in blocks:
        parent, number, extrinsics = decode_block(path.read_bytes())
        if previous is None and number != CHECKPOINT_NUMBER + 1:
            raise Halt("replay does not start at the first child")
        if previous is not None and number != previous + 1:
            raise Halt(f"replay gap before block {number}")
        if number == CHECKPOINT_NUMBER + 1:
            if parent != CHECKPOINT_HASH or not any(e.endswith(TICK) for e in extrinsics):
                raise Halt("first child is not a Tick on the checkpoint")
            ticks += 1
        else:
            expected = envelopes[included] if included < len(envelopes) else None
            if advances(extrinsics, expected, str(number)):
                included += 1
            elif expected is not None:
                raise Halt(f"block {number} does not apply bundle envelope {included}")
            else:
                ordinary += 1
        previous = number
    return {"blocks": len(blocks), "firstChildTick": ticks == 1, "envelopesApplied": included,
            "bundleEnvelopes": len(envelopes), "ordinaryBlocksAfterCompletion": ordinary,
            "complete": included == len(envelopes)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    feed = sub.add_parser("run", help="deliver work to a running node's inherent directory")
    feed.add_argument("--bundle", type=Path, required=True)
    feed.add_argument("--inherent-dir", type=Path, required=True)
    feed.add_argument("--rpc", default="http://127.0.0.1:9944")
    feed.add_argument("--poll", type=float, default=0.5)
    check = sub.add_parser("verify-replay", help="check the bundle against rehearsal blocks")
    check.add_argument("--bundle", type=Path, required=True)
    check.add_argument("--replay", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "run":
        sys.exit(run(args))
    print(json.dumps(verify_replay(args.bundle, args.replay), indent=2))
