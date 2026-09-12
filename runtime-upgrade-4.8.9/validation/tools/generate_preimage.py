#!/usr/bin/env python3
"""
Generate a SCALE-encoded runtime upgrade call (System.set_code / set_code_without_checks),
and print the preimage hash + length expected by Democracy external proposals.

This script is intentionally offline (no node connection required).
It assumes the runtime keeps:
  - System pallet index = 0
  - frame_system::Call::set_code index = 2
  - frame_system::Call::set_code_without_checks index = 3

Those indices are true for this repo on polkadot-v0.9.38 and can be verified with:
  NO_COLOR=true subwasm metadata --format json <wasm> | jq ...
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Dict, Tuple


def scale_compact_u32(value: int) -> bytes:
    if value < 0:
        raise ValueError("compact encoding only supports non-negative integers")

    # SCALE compact encoding:
    # - [0, 2^6)      -> single byte: (value << 2)
    # - [2^6, 2^14)   -> two bytes  : (value << 2) | 0b01
    # - [2^14, 2^30)  -> four bytes : (value << 2) | 0b10
    # - [2^30, ...)   -> big int    : 0b11 + LE bytes length marker
    if value < (1 << 6):
        return bytes([(value << 2) | 0b00])
    if value < (1 << 14):
        return ((value << 2) | 0b01).to_bytes(2, "little")
    if value < (1 << 30):
        return ((value << 2) | 0b10).to_bytes(4, "little")

    le = value.to_bytes((value.bit_length() + 7) // 8, "little")
    if len(le) < 4:
        le = le + b"\x00" * (4 - len(le))
    if len(le) > 67:
        raise ValueError("compact encoding too large (len > 67)")
    prefix = (((len(le) - 4) << 2) | 0b11) & 0xFF
    return bytes([prefix]) + le


def blake2_256(data: bytes) -> bytes:
    h = hashlib.blake2b(digest_size=32)
    h.update(data)
    return h.digest()


def stream_blake2_256(chunks) -> bytes:
    h = hashlib.blake2b(digest_size=32)
    for c in chunks:
        h.update(c)
    return h.digest()


def system_set_code_call_indices(without_checks: bool) -> Tuple[int, int]:
    # System pallet index in this runtime is 0.
    pallet_index = 0
    call_index = 3 if without_checks else 2
    return pallet_index, call_index


def generate_call_bytes(wasm_bytes: bytes, without_checks: bool) -> Tuple[bytes, Dict[str, int]]:
    pallet_index, call_index = system_set_code_call_indices(without_checks)
    prefix = bytes([pallet_index, call_index])
    code_len = len(wasm_bytes)
    code_len_compact = scale_compact_u32(code_len)
    call_len = len(prefix) + len(code_len_compact) + code_len
    return prefix + code_len_compact + wasm_bytes, {"code_len": code_len, "call_len": call_len}


def main() -> None:
    ap = argparse.ArgumentParser(
        prog="generate_preimage",
        description="Generate runtime upgrade call preimage (hash + len) for council/democracy.",
    )
    ap.add_argument(
        "--wasm",
        required=True,
        help="Path to runtime wasm artifact (typically framenode_runtime.compact.compressed.wasm)",
    )
    ap.add_argument(
        "--without-checks",
        action="store_true",
        help="Use System.set_code_without_checks instead of System.set_code",
    )
    ap.add_argument(
        "--out",
        help="Output path for raw SCALE-encoded call bytes (binary). Default: <wasm>.preimage.call",
    )
    ap.add_argument(
        "--json-out",
        help="Output path for JSON summary. Default: <wasm>.preimage.json",
    )
    args = ap.parse_args()

    wasm_path = Path(args.wasm)
    wasm_bytes = wasm_path.read_bytes()
    wasm_len = len(wasm_bytes)
    wasm_hash = blake2_256(wasm_bytes).hex()

    pallet_index, call_index = system_set_code_call_indices(args.without_checks)
    code_len_compact = scale_compact_u32(wasm_len)

    # Stream hash computation to avoid building a second ~3MB buffer.
    call_hash = stream_blake2_256([bytes([pallet_index, call_index]), code_len_compact, wasm_bytes]).hex()
    call_len = 2 + len(code_len_compact) + wasm_len

    out_path = Path(args.out) if args.out else wasm_path.with_suffix(wasm_path.suffix + ".preimage.call")
    json_out_path = Path(args.json_out) if args.json_out else wasm_path.with_suffix(wasm_path.suffix + ".preimage.json")

    # Write raw call bytes (for tooling that accepts a file input).
    with out_path.open("wb") as f:
        f.write(bytes([pallet_index, call_index]))
        f.write(code_len_compact)
        f.write(wasm_bytes)

    summary = {
        "wasm_path": str(wasm_path),
        "wasm_len": wasm_len,
        "wasm_blake2_256": "0x" + wasm_hash,
        "call": {
            "pallet_index": pallet_index,
            "call_index": call_index,
            "variant": "set_code_without_checks" if args.without_checks else "set_code",
        },
        # What Democracy expects:
        "proposal_hash": "0x" + call_hash,
        "proposal_len": call_len,
        "call_bytes_file": str(out_path),
    }

    json_out_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()

